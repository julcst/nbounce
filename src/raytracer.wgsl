const COMPUTE_SIZE: u32 = 8u;
const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 2.0 * PI;
const MAX_FLOAT: f32 = 0x1.fffffep+127f;
const NO_HIT: f32 = MAX_FLOAT;
const EPS: f32 = 0.00000001;
const STACK_SIZE = 32u;

// TODO: Move to uniforms
const MAX_BOUNCES = 8u;
const MIN_THROUGHPUT_SQ = 0.0001;

@group(0) @binding(0) var output: texture_storage_2d<rgba32float, read_write>;

struct CameraData {
    world_to_clip: mat4x4f,
    clip_to_world: mat4x4f,
};

@group(1) @binding(0) var<uniform> camera: CameraData;

struct BVHNode {
    min: vec3f,
    start: u32,
    max: vec3f,
    end: u32,
};

struct Instance {
    world_to_local: mat4x4f,
    local_to_world: mat4x4f,
    color: vec4f,
    roughness: f32,
    metallic: f32,
    emissive: f32,
    node: u32,
};

struct Vertex {
    position: vec3f,
    u: f32,
    normal: vec3f,
    v: f32,
    tangent: vec4f,
};

@group(2) @binding(0) var<storage, read> blas: array<BVHNode>;
@group(2) @binding(1) var<storage, read> tlas: array<BVHNode>;
@group(2) @binding(2) var<storage, read> instances: array<Instance>;
@group(2) @binding(3) var<storage, read> vertices: array<Vertex>;
@group(2) @binding(4) var<storage, read> indices: array<u32>;
@group(2) @binding(5) var environment: texture_cube<f32>;
@group(2) @binding(6) var environment_sampler: sampler;

struct PushConstants {
    frame: u32,
    sample_count: f32,
    weight: f32,
};

var<push_constant> c: PushConstants;

struct Ray {
    origin: vec3f,
    direction: vec3f,
    inv_direction: vec3f,
};

struct AABB {
    min: vec3f,
    max: vec3f,
};

// Möller–Trumbore intersection algorithm
fn intersect_triangle(ray: Ray, v0: vec3f, v1: vec3f, v2: vec3f) -> vec3f {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = cross(ray.direction, edge2);
    let det = dot(edge1, h);
    // TODO: Do det > -EPS for inner refraction rays
    if det < EPS { // Backface culling: det < EPS
        return vec3f(NO_HIT, NO_HIT, NO_HIT); // Parallel or culled
    }
    var inv_det = 1 / det;
    var s = ray.origin - v0;
    var u = inv_det * dot(s, h);
    if u < 0 || u > 1 {
        return vec3f(NO_HIT, NO_HIT, NO_HIT); // Outside
    }
    var q = cross(s, edge1);
    var v = inv_det * dot(ray.direction, q);
    if v < 0 || u + v > 1 {
        return vec3f(NO_HIT, NO_HIT, NO_HIT); // Outside
    }
    var t = inv_det * dot(edge2, q);
    // TODO: Fix near plane for reflections
    if t < 0.0 {
        return vec3f(NO_HIT, NO_HIT, NO_HIT); // Behind
    }
    return vec3f(t, u, v);
}

// From https://tavianator.com/2022/ray_box_boundary.html
fn intersect_AABB(ray: Ray, aabb: AABB) -> f32 {
    var t_min = (aabb.min - ray.origin) * ray.inv_direction;
    var t_max = (aabb.max - ray.origin) * ray.inv_direction;
    var t_1 = min(t_min, t_max);
    var t_2 = max(t_min, t_max);
    var t_near = max(t_1.x, max(t_1.y, t_1.z));
    var t_far = min(t_2.x, min(t_2.y, t_2.z));
    return select(NO_HIT, t_near, t_near <= t_far && t_far >= 0.0);
}

const EMISSIVE = 1u;

struct HitInfo {
    position: vec3f,
    dist: f32,
    normal: vec3f,
    n_aabb: u32,
    texcoord: vec2f,
    n_tri: u32,
    roughness: f32,
    color: vec4f,
    tangent: vec4f,
    metallic: f32,
    flags: u32,
};

fn no_hit_info() -> HitInfo {
    return HitInfo(vec3f(0.0), NO_HIT, vec3f(0.0), 0u, vec2f(0.0), 0u, 0.0, vec4f(0.0), vec4f(0.0), 0.0, 0u);
}

struct StackEntry {
    index: u32,
    dist: f32,
};

// TODO: Implement HW raytracing
// TODO: Refactor in sperate file
fn intersect_TLAS(ray: Ray) -> HitInfo {
    var stack: array<StackEntry, STACK_SIZE>;

    var hit = no_hit_info();

    // Init stack with top node
    var i = 0u;
    let index_top = 0u;
    let top = tlas[index_top];
    let dist_top = intersect_AABB(ray, AABB(top.min, top.max));
    hit.n_aabb += 1u;
    if dist_top < hit.dist {
        stack[i] = StackEntry(index_top, dist_top);
        i += 1u;
    }

    while i > 0u {
        // Pop next node from stack
        i -= 1u;
        var stack_entry = stack[i];
        if stack_entry.dist >= hit.dist { continue; } // Skip if node is farther than current hit
        var node = tlas[stack_entry.index];
        let is_leaf = node.end > 0u;
        if is_leaf { // Leaf node
            for (var j = node.start; j < node.end; j += 1u) {
                let instance = instances[j];
                let local_origin = instance.world_to_local * vec4f(ray.origin, 1.0);
                let local_direction = mat3(instance.world_to_local) * ray.direction;
                let local_ray = Ray(local_origin.xyz, local_direction, 1.0 / local_direction);
                let hit_local = intersect_BLAS(local_ray, instance.node);
                hit.n_aabb += hit_local.n_aabb;
                hit.n_tri += hit_local.n_tri;
                if hit_local.dist < hit.dist {
                    hit.dist = hit_local.dist;
                    hit.position = ray.origin + hit.dist * ray.direction;
                    hit.normal = normalize(mat3(instance.local_to_world) * hit_local.normal);
                    hit.tangent = vec4f(mat3(instance.local_to_world) * hit_local.tangent.xyz, hit_local.tangent.w);
                    hit.texcoord = hit_local.texcoord;
                    hit.color = instance.color;
                    hit.roughness = instance.roughness;
                    hit.metallic = instance.metallic;
                }
            }
        } else {
            let index_left = node.start;
            let left_node = tlas[index_left];
            let left = StackEntry(index_left, intersect_AABB(ray, AABB(left_node.min, left_node.max)));

            let index_right = index_left + 1u;
            let right_node = tlas[index_right];
            let right = StackEntry(index_right, intersect_AABB(ray, AABB(right_node.min, right_node.max)));
            
            hit.n_aabb += 2u;

            var far = left;
            var near = right;
            if left.dist < right.dist {
                far = right;
                near = left;
            }

            // Look at far node last
            if far.dist < hit.dist {
                stack[i] = far;
                i += 1u;
            }

            // Look at near node first
            if near.dist < hit.dist {
                stack[i] = near;
                i += 1u;
            }
        }
    }
    if hit.dist == NO_HIT {
        // TODO: Use pre-filtered mipmaps
        hit.color = textureSampleLevel(environment, environment_sampler, ray.direction, 0.0);
        hit.flags |= EMISSIVE;
    }
    return hit;
}

fn intersect_BLAS(ray: Ray, index_top: u32) -> HitInfo {
    var stack: array<StackEntry, STACK_SIZE>;

    var hit = no_hit_info();

    // Init stack with top node
    var i = 0u;
    let top = blas[index_top];
    let dist_top = intersect_AABB(ray, AABB(top.min, top.max));
    hit.n_aabb += 1u;
    if dist_top < hit.dist {
        stack[i] = StackEntry(index_top, dist_top);
        i += 1u;
    }

    while i > 0u {
        // Pop next node from stack
        i -= 1u;
        var stack_entry = stack[i];
        if stack_entry.dist >= hit.dist { continue; } // Skip if node is farther than current hit
        var node = blas[stack_entry.index];
        let is_leaf = node.end > 0u;
        if is_leaf { // Leaf node
            for (var j = node.start * 3u; j < node.end * 3u; j += 3u) {
                let v0 = vertices[indices[j + 0u]];
                let v1 = vertices[indices[j + 1u]];
                let v2 = vertices[indices[j + 2u]];
                let t = intersect_triangle(ray, v0.position, v1.position, v2.position);
                hit.n_tri += 1u;
                if t.x < hit.dist {
                    hit.dist = t.x;
                    let barycentrics = vec3f(1.0 - t.y - t.z, t.yz);
                    hit.position = ray.origin + hit.dist * ray.direction;
                    hit.normal = normalize(mat3x3f(v0.normal, v1.normal, v2.normal) * barycentrics);
                    // TODO: Benchmark?
                    // hit.texcoord = barycentrics * mat2x3f(vec3f(v0.u, v1.u, v2.u), vec3f(v0.v, v1.v, v2.v)); // 16.7 44.7
                    hit.texcoord = mat3x2f(vec2f(v0.u, v0.v), vec2f(v1.u, v1.v), vec2f(v2.u, v2.v)) * barycentrics; // 16.4 44.3
                    hit.tangent = mat3x4f(v0.tangent, v1.tangent, v2.tangent) * barycentrics;
                }
            }
        } else {
            let index_left = node.start;
            let left_node = blas[index_left];
            let left = StackEntry(index_left, intersect_AABB(ray, AABB(left_node.min, left_node.max)));

            let index_right = index_left + 1u;
            let right_node = blas[index_right];
            let right = StackEntry(index_right, intersect_AABB(ray, AABB(right_node.min, right_node.max)));
            
            hit.n_aabb += 2u;

            var far = left;
            var near = right;
            if left.dist < right.dist {
                far = right;
                near = left;
            }

            // Look at far node last
            if far.dist < hit.dist {
                stack[i] = far;
                i += 1u;
            }

            // Look at near node first
            if near.dist < hit.dist {
                stack[i] = near;
                i += 1u;
            }
        }
    }
    return hit;
}

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

fn mat3(m: mat4x4f) -> mat3x3f {
    return mat3x3f(m[0].xyz, m[1].xyz, m[2].xyz);
}

fn intersect_instances(ray: Ray) -> HitInfo {
    var hit = no_hit_info();
    for (var j = 0u; j < arrayLength(&instances); j += 1u) {
        let instance = instances[j];
        let local_origin = instance.world_to_local * vec4f(ray.origin, 1.0);
        let local_direction = mat3(instance.world_to_local) * ray.direction;
        let local_ray = Ray(local_origin.xyz, local_direction.xyz, 1.0 / local_direction.xyz);
        let hit_local = intersect_BLAS(local_ray, instance.node);
        hit.n_aabb += hit_local.n_aabb;
        hit.n_tri += hit_local.n_tri;
        if hit_local.dist < hit.dist {
            hit.dist = hit_local.dist;
            hit.position = ray.origin + hit.dist * ray.direction;
            hit.normal = normalize((instance.world_to_local * vec4f(hit_local.normal, 0.0)).xyz);
            hit.texcoord = hit_local.texcoord;
        }
    }
    return hit;
}

/// Sample visible normal distribution function using the algorithm
/// from "Sampling Visible GGX Normals with Spherical Caps" by Dupuy et al. 2023.
/// https://cdrdv2-public.intel.com/782052/sampling-visible-ggx-normals.pdf
fn sample_vndf(rand: vec2f, wi: vec3f, alpha: vec2f) -> vec3f {
    // warp to the hemisphere configuration
    let wiStd = normalize(vec3f(wi.xy * alpha, wi.z));
    // sample a spherical cap in (-wi.z, 1]
    let phi = TWO_PI * rand.x;
    let z = fma((1.0 - rand.y), (1.0 + wiStd.z), -wiStd.z);
    let sinTheta = sqrt(clamp(1.0 - z * z, 0.0, 1.0));
    let x = sinTheta * cos(phi);
    let y = sinTheta * sin(phi);
    // compute halfway direction as standard normal
    let wmStd = vec3(x, y, z) + wiStd;
    // warp back to the ellipsoid configuration
    let wm = normalize(vec3f(wmStd.xy * alpha, wmStd.z));
    // return final normal
    return wm;
}

fn sample_vndf_iso(rand: vec2f, wi: vec3f, alpha: f32, n: vec3f) -> vec3f {
    // decompose the vector in parallel and perpendicular components
    let wi_z = n * dot(wi, n);
    let wi_xy = wi - wi_z;
    // warp to the hemisphere configuration
    let wiStd = normalize(wi_z - alpha * wi_xy);
    // sample a spherical cap in (-wiStd.z, 1]
    let wiStd_z = dot(wiStd, n);
    let phi = (2.0 * rand.x - 1.0) * PI;
    let z = (1.0 - rand.y) * (1.0 + wiStd_z) - wiStd_z;
    let sinTheta = sqrt(clamp(1.0 - z * z, 0.0, 1.0));
    let x = sinTheta * cos(phi);
    let y = sinTheta * sin(phi);
    let cStd = vec3(x, y, z);
    // reflect sample to align with normal
    let up = vec3f(0, 0, 1);
    var wr = n + up;
    if wr.z == 0.0 { wr.z = 0.0000001; } // TODO: Find better solution
    let c = dot(wr, cStd) * wr / wr.z - cStd;
    // compute halfway direction as standard normal
    let wmStd = c + wiStd;
    let wmStd_z = n * dot(n, wmStd);
    let wmStd_xy = wmStd_z - wmStd;
    // warp back to the ellipsoid configuration
    let wm = normalize(wmStd_z + alpha * wmStd_xy);
    // return final normal
    return wm;
}

fn sample_uniform(rand: vec2f) -> vec3f {
    return vec3f(0.0); // TODO
}

fn hash4u(s: vec4u) -> vec4u {
    var v = s * 1664525u + 1013904223u;
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
    v ^= v >> vec4u(16u);
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
    return v;
}

fn hash4f(s: vec4u) -> vec4f {
    return vec4f(hash4u(s)) * (1.0 / f32(0xffffffffu));
}

/**
 * Schlick's approximation for the Fresnel term (see https://en.wikipedia.org/wiki/Schlick%27s_approximation).
 * The Fresnel term describes how light is reflected at the surface.
 * For conductors the reflection coefficient R0 is chromatic, for dielectrics it is achromatic.
 * R0 = ((n1 - n2) / (n1 + n2))^2 with n1, n2 being the refractive indices of the two materials.
 * We can set n1 = 1.0 (air) and n2 = IoR of the material.
 * Most dielectrics have an IoR near 1.5 => R0 = ((1 - 1.5) / (1 + 1.5))^2 = 0.04.
 */
fn F_SchlickApprox(HdotV: f32, R0: vec3f) -> vec3f {
    return R0 + (1.0 - R0) * pow(1.0 - HdotV, 5.0);
}

/**
 * Lambda for the Trowbridge-Reitz NDF
 * Measures invisible masked microfacet area per visible microfacet area.
 */
fn Lambda_TrowbridgeReitz(NdotV: f32, alpha2: f32) -> f32 {
    let cosTheta = NdotV;
    let cos2Theta = cosTheta * cosTheta;
    let sin2Theta = 1.0 - cos2Theta;
    let tan2Theta = sin2Theta / cos2Theta;
    return (-1.0 + sqrt(1.0 + alpha2 * tan2Theta)) / 2.0;
}

/**
 * Smith's shadowing-masking function for the Trowbridge-Reitz NDF.
 */
fn G2_TrowbridgeReitz(NdotL: f32, NdotV: f32, alpha2: f32) -> f32 {
    let lambdaL = Lambda_TrowbridgeReitz(NdotL, alpha2);
    let lambdaV = Lambda_TrowbridgeReitz(NdotV, alpha2);
    return 1.0 / (1.0 + lambdaL + lambdaV);
}

/**
 * Smith's shadowing-masking function for the Trowbridge-Reitz NDF.
 */
fn G1_TrowbridgeReitz(NdotV: f32, alpha2: f32) -> f32 {
    let lambdaV = Lambda_TrowbridgeReitz(NdotV, alpha2);
    return 1.0 / (1.0 + lambdaV);
}

fn sample_rendering_eq(rand: vec2f, dir: Ray) -> vec3f {
    var throughput = vec3f(1.0);
    var ray = dir;
    for (var bounces = 0u; bounces <= MAX_BOUNCES; bounces += 1u) {
        let hit = intersect_TLAS(ray);
        if (hit.flags & EMISSIVE) != 0u {
            return throughput * hit.color.xyz;
        }
        // TODO: Multiple Importance Sampling?
        let alpha = hit.roughness * hit.roughness;
        let alpha2 = alpha * alpha;
        let wo = normalize(-ray.direction);
        let wn = sample_vndf_iso(rand, wo, alpha, hit.normal);
        let wi = reflect(-wo, wn);
        let NdotL = dot(wi, hit.normal);
        let NdotV = dot(wo, hit.normal);
        let F = F_SchlickApprox(dot(wi, wn), hit.color.xyz);
        let LambdaL = Lambda_TrowbridgeReitz(NdotL, alpha2);
        let LambdaV = Lambda_TrowbridgeReitz(NdotV, alpha2);
        let specular = F * (1.0 + LambdaV) / (1.0 + LambdaL + LambdaV); // = F * (G2 / G1)
        throughput *= specular;
        if dot(throughput, throughput) <= MIN_THROUGHPUT_SQ { break; }
        ray = Ray(hit.position, wi, 1.0 / wi);
        // let t = hit.tangent.xyz;
        // let b = hit.tangent.w * cross(hit.tangent.xyz, hit.normal);
        // let n = hit.normal;
        // let tbn = mat3x3f(t, b, n);
        // let wo = normalize(ray.direction * tbn);
        // let alpha = hit.roughness * hit.roughness;
        // let wn = sample_vndf(rand, -wo, vec2f(alpha));
        // let wi = reflect(wo, wn);
        // throughput *= hit.color.xyz;
        // if dot(throughput, throughput) <= MIN_THROUGHPUT_SQ { break; }
        // let world_wi = normalize(tbn * wi);
        // ray = Ray(hit.position, world_wi, 1.0 / world_wi);
    }
    return vec3f(0.0);
}

@compute
@workgroup_size(COMPUTE_SIZE, COMPUTE_SIZE)
fn main(@builtin(global_invocation_id) id: vec3u) {
    var color = vec4f(0.0);
    if c.sample_count > 1 {
        color = textureLoad(output, vec2i(id.xy));
    }

    let rand = hash4f(vec4u(id.xyz, c.frame));

    let ray = generate_ray(id, rand.xy);

    // FIXME: Why does this progressively get darker, floating point errors?
    let sample = sample_rendering_eq(rand.zw, ray);
    color = vec4f(mix(color.xyz, sample, c.weight), 1.0);

    textureStore(output, id.xy, color);

    // let hit = intersect_TLAS(ray);
    // textureStore(output, id.xy, vec4f(hash2f(id.xy), 0.0, 1.0));
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