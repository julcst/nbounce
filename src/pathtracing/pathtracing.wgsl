const COMPUTE_SIZE: u32 = 8u;
const LDS_PER_BOUNCE: u32 = 2u;
// TODO: Move to a push constant
const LDS_STRIDE = 8 * LDS_PER_BOUNCE + 1u;

struct CameraData {
    world_to_clip: mat4x4f,
    clip_to_world: mat4x4f,
};

@group(0) @binding(0) var output: texture_storage_2d<rgba32float, read_write>;
@group(0) @binding(1) var<uniform> camera: CameraData;
@group(0) @binding(2) var<storage, read> sobol_burley: array<vec4f>;
@group(0) @binding(3) var environment: texture_cube<f32>;
@group(0) @binding(4) var environment_sampler: sampler;

struct PushConstants {
    sample: u32,
    weight: f32,
    bounces: u32,
    contribution_factor: f32,
};

var<push_constant> c: PushConstants;

// TODO: Match with rasterization
fn generate_ray(id: vec3u, rand: vec2f) -> Ray {
    let dim = vec2f(textureDimensions(output));
    let uv = 2.0 * (vec2f(id.xy) + rand) / dim - 1.0;

    // let clip_pos = vec4f(0.0, 0.0, 0.0, 1.0);
    // let world_pos = camera.clip_to_world * clip_pos;
    let world_pos = camera.clip_to_world[3]; // Equivalent to the above
    let pos = world_pos.xyz / world_pos.w;

    let clip_dir = vec4f(-uv, -1.0, 1.0);
    let world_dir = camera.clip_to_world * clip_dir;
    let dir = pos - world_dir.xyz / world_dir.w;

    return Ray(pos, dir, 1.0 / dir);
}

/// Sample visible normal distribution function using the algorithm
/// from "Sampling Visible GGX Normals with Spherical Caps" by Dupuy et al. 2023.
/// https://cdrdv2-public.intel.com/782052/sampling-visible-ggx-normals.pdf
/// Implementation from https://gist.github.com/jdupuy/4c6e782b62c92b9cb3d13fbb0a5bd7a0
fn sample_vndf(rand: vec2f, wi: vec3f, alpha: vec2f) -> vec3f {
    // Warp to the hemisphere configuration
    let wiStd = normalize(vec3f(wi.xy * alpha, wi.z));
    // Sample a spherical cap in (-wi.z, 1]
    let phi = TWO_PI * rand.x;
    let z = fma((1.0 - rand.y), (1.0 + wiStd.z), -wiStd.z);
    let sinTheta = sqrt(clamp(1.0 - z * z, 0.0, 1.0));
    let x = sinTheta * cos(phi);
    let y = sinTheta * sin(phi);
    // Compute halfway direction as standard normal
    let wmStd = vec3(x, y, z) + wiStd;
    // Warp back to the ellipsoid configuration
    let wm = normalize(vec3f(wmStd.xy * alpha, wmStd.z));
    // Return final normal
    return wm;
}

/// Sample visible normal distribution function using the algorithm
/// from "Sampling Visible GGX Normals with Spherical Caps" by Dupuy et al. 2023.
/// https://cdrdv2-public.intel.com/782052/sampling-visible-ggx-normals.pdf
/// Implementation from https://gist.github.com/jdupuy/4c6e782b62c92b9cb3d13fbb0a5bd7a0
fn sample_vndf_iso(rand: vec2f, wi: vec3f, alpha: f32, n: vec3f) -> vec3f {
    // Note: Else produces NaN for alpha = 0
    if alpha == 0.0 { return n; }
    // Decompose the vector in parallel and perpendicular components
    let wi_z = n * dot(wi, n);
    let wi_xy = wi - wi_z;
    // Warp to the hemisphere configuration
    let wiStd = normalize(wi_z - alpha * wi_xy);
    // Sample a spherical cap in (-wiStd.z, 1]
    let wiStd_z = dot(wiStd, n);
    let phi = (2.0 * rand.x - 1.0) * PI;
    let z = (1.0 - rand.y) * (1.0 + wiStd_z) - wiStd_z;
    let sinTheta = sqrt(clamp(1.0 - z * z, 0.0, 1.0));
    let x = sinTheta * cos(phi);
    let y = sinTheta * sin(phi);
    let cStd = vec3(x, y, z);
    // Reflect sample to align with normal
    let up = vec3f(0, 0, 1);
    let wr = n + up;
    // Prevent division by zero
    let safe_wrz = max(wr.z, 1e-6);
    let c = dot(wr, cStd) * wr / safe_wrz - cStd;
    // Compute halfway direction as standard normal
    let wmStd = c + wiStd;
    let wmStd_z = n * dot(n, wmStd);
    let wmStd_xy = wmStd_z - wmStd;
    // Warp back to the ellipsoid configuration
    let wm = normalize(wmStd_z + alpha * wmStd_xy);
    // Return final normal
    return wm;
}

fn sample_cosine_hemisphere(rand: vec2f) -> vec3f {
    let phi = TWO_PI * rand.x;
    let sinTheta = sqrt(1.0 - rand.y);
    let cosTheta = sqrt(rand.y);
    return vec3f(cos(phi) * sinTheta, sin(phi) * sinTheta, cosTheta);
}

/// Schlick's approximation for the Fresnel term (see https://en.wikipedia.org/wiki/Schlick%27s_approximation).
/// The Fresnel term describes how light is reflected at the surface.
/// For conductors the reflection coefficient R0 is chromatic, for dielectrics it is achromatic.
/// R0 = ((n1 - n2) / (n1 + n2))^2 with n1, n2 being the refractive indices of the two materials.
/// We can set n1 = 1.0 (air) and n2 = IoR of the material.
/// Most dielectrics have an IoR near 1.5 => R0 = ((1 - 1.5) / (1 + 1.5))^2 = 0.04.
fn F_SchlickApprox(HdotV: f32, R0: vec3f) -> vec3f {
    return R0 + (1.0 - R0) * pow(1.0 - HdotV, 5.0);
}

/// Lambda for the Trowbridge-Reitz NDF
/// Measures invisible masked microfacet area per visible microfacet area.
fn Lambda_TrowbridgeReitz(NdotV: f32, alpha2: f32) -> f32 {
    let cosTheta = NdotV;
    let cos2Theta = cosTheta * cosTheta;
    let sin2Theta = 1.0 - cos2Theta;
    let tan2Theta = sin2Theta / cos2Theta;
    return (-1.0 + sqrt(1.0 + alpha2 * tan2Theta)) / 2.0;
}

/// Smith's shadowing-masking function for the Trowbridge-Reitz NDF.
fn G2_TrowbridgeReitz(NdotL: f32, NdotV: f32, alpha2: f32) -> f32 {
    let lambdaL = Lambda_TrowbridgeReitz(NdotL, alpha2);
    let lambdaV = Lambda_TrowbridgeReitz(NdotV, alpha2);
    return 1.0 / (1.0 + lambdaL + lambdaV);
}

/// Smith's shadowing-masking function for the Trowbridge-Reitz NDF.
fn G1_TrowbridgeReitz(NdotV: f32, alpha2: f32) -> f32 {
    let lambdaV = Lambda_TrowbridgeReitz(NdotV, alpha2);
    return 1.0 / (1.0 + lambdaV);
}

/// From http://mikktspace.com to apply normal maps
fn mikktspace(hit: HitInfo) -> mat3x3f {
    let n = hit.normal;
    let t = hit.tangent.xyz;
    let b = hit.tangent.w * cross(n, t);
    return mat3x3f(t, b, n);
}

/// Takes a precomputed Sobol-Burley sample and performs a Cranly-Patterson-Rotation with a per pixel shift.
/// For each sample the precomputed Sobol-Burley array contains first one vec4f for lens and pixel sampling 
/// and then two vec4f for each bounce.
fn sample_sobol_burley_bounce(i: u32, bounce: u32, shift: vec4f, dim: u32) -> vec4f {
    let sample = sobol_burley[i * LDS_STRIDE + 1u + bounce * LDS_PER_BOUNCE + dim];
    return fract(sample + shift);
}

fn sample_sobol_burley_extra(i: u32, shift: vec4f) -> vec4f {
    let sample = sobol_burley[i * LDS_STRIDE];
    return fract(sample + shift);
}

fn sample_rendering_eq(sample: u32, shift: vec4f, dir: Ray) -> vec3f {
    var throughput = vec3f(1.0);
    var ray = dir;
    for (var bounce = 0u; bounce <= c.bounces; bounce += 1u) {
        let hit = intersect_scene(ray);

        if hit.dist == NO_HIT {
            let env_color = textureSampleLevel(environment, environment_sampler, ray.direction, 0.0).xyz;
            return throughput * env_color;
        }

        if (hit.flags & EMISSIVE) != 0u {
            return throughput * hit.color.xyz;
        }

        // Collect hit info
        let alpha = hit.roughness * hit.roughness;
        let alpha2 = alpha * alpha;
        let n = normalize(hit.normal);

        // Collect bounce info
        let sobol_0 = sample_sobol_burley_bounce(sample, bounce, shift, 0u);
        let sobol_1 = sample_sobol_burley_bounce(sample, bounce, shift, 1u);
        let wo = normalize(-ray.direction);
        let cosThetaO = dot(wo, n);
        var wi: vec3f;

        let metallic = hit.metallic;
        let albedo = hit.color.xyz;

        // TODO: Importance Sample environment map

        // TODO: Importance Sample using the complete BRDF
        let F0 = mix(vec3f(0.04), albedo, metallic);
        let specular_weight = luminance(F_SchlickApprox(dot(wo, n), F0));
        let diffuse_weight = (1.0 - metallic) * luminance(albedo);

        let p_specular = specular_weight / (specular_weight + diffuse_weight);
        let p_diffuse = 1.0 - p_specular;

        // Precomputed texture for BRDF mean for importance sampling
        if sobol_0.x < p_specular { // Trowbridge-Reitz-Specular
            let wm = sample_vndf_iso(sobol_0.yz, wo, alpha, n); // Sample microfacet normal after Trowbridge-Reitz VNDF
            wi = reflect(-wo, wm);
            let cosThetaD = dot(wo, wm); // = dot(wi, wm)
            let cosThetaI = dot(wi, n);
            let F = F_SchlickApprox(cosThetaD, F0);
            let LambdaL = Lambda_TrowbridgeReitz(cosThetaI, alpha2);
            let LambdaV = Lambda_TrowbridgeReitz(cosThetaO, alpha2);
            let specular = F * (1 + LambdaV) / (1 + LambdaL + LambdaV); // = F * (G2 / G1)
            throughput *= specular / p_specular;
        } else { // Brent-Burley-Diffuse
            let tangent_to_world = build_tbn(n, hit.tangent.xyz);
            wi = tangent_to_world * sample_cosine_hemisphere(sobol_1.yz);
            let wm = normalize(wi + wo); // Microfacect normal is the half vector
            let cosThetaD = dot(wi, wm); // = dot(wo, wm)
            let cosThetaI = dot(wi, n);
            let FD90 = 0.5 + 2 * alpha * pow(cosThetaD, 2.0);
            let response = (1 + (FD90 - 1) * pow(1 - cosThetaI, 5.0)) * (1 + (FD90 - 1) * pow(1 - cosThetaO, 5.0));
            // Note: We drop the 1.0 / PI prefactor
            let diffuse = (1 - metallic) * albedo * response;
            throughput *= diffuse / p_diffuse;
        }

        // Unbiased Russian Roulette path termination
        // Start with 1.0 then gradually decrease to 0.0
        var p_continue = min(1.0 - pow(f32(bounce) / f32(c.bounces), 8.0), 1.0);

        // Terminate also if the perceived throughput becomes too low
        p_continue *= min(luminance(throughput) * c.contribution_factor, 1.0);

        if sobol_0.z < p_continue {
            throughput /= p_continue;
        } else {
            return vec3f(0.0);
        }

        ray = Ray(hit.position, wi, 1.0 / wi);
    }
    return vec3f(0.0);
}

@compute
@workgroup_size(COMPUTE_SIZE, COMPUTE_SIZE)
fn main(@builtin(global_invocation_id) id: vec3u) {
    var color = vec4f(0.0);
    if c.weight > 0.0 {
        color = textureLoad(output, vec2i(id.xy));
    }

    // TODO: Read from texture
    let shift = hash4f(id.xyxy);

    let jitter = sample_sobol_burley_extra(c.sample, shift);
    let ray = generate_ray(id, jitter.xy);

    let sample = sample_rendering_eq(c.sample, shift, ray);
    color = vec4f(mix(color.xyz, sample, c.weight), 1.0);

    textureStore(output, id.xy, color);

    // let hit = intersect_TLAS(ray);
    // textureStore(output, id.xy, vec4f(f32(hit.n_aabb) * 0.02, select(0.0, 1.0, hit.dist != NO_HIT), f32(hit.n_tri) * 0.2, 1.0));
    // textureStore(output, id.xy, hit.color);
    // textureStore(output, id.xy, vec4f(vec3f(hit.roughness), 1.0));
    // textureStore(output, id.xy, vec4f(vec3f(hit.metallic), 1.0));
    // textureStore(output, id.xy, vec4f(hit.tangent.xyz * 0.5 + 0.5, 1.0));
    // textureStore(output, id.xy, vec4f(vec3f(hit.tangent.w) * 0.5 + 0.5, 1.0));
    // textureStore(output, id.xy, vec4f(hit.normal * 0.5 + 0.5, 1.0));
    // textureStore(output, id.xy, vec4f(hit.texcoord, 0.0, 1.0));
    // textureStore(output, id.xy, vec4f(hit.position, 1.0));
}