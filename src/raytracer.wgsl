@group(0)
@binding(0)
var output: texture_storage_2d<rgba16float, read_write>;

struct CameraData {
    world_to_clip: mat4x4f,
    clip_to_world: mat4x4f,
};

@group(1)
@binding(0)
var<uniform> camera: CameraData;

const COMPUTE_SIZE = 16.0;
const PI: f32 = 3.14159265359;
const NO_HIT: f32 = -1.0;
const FOV: f32 = PI / 3.0;
const FOCAL_LENGTH: f32  = 1.0 / tan(FOV / 2.0);

struct Ray {
    origin: vec3f,
    direction: vec3f,
    inv_direction: vec3f,
};

struct AABB {
    min: vec3f,
    max: vec3f,
};

const aabb = AABB(vec3f(-1.0, -1.0, -1.0), vec3f(1.0, 1.0, 1.0));

// Möller–Trumbore intersection algorithm
fn intersect_triangle(ray: Ray, v0: vec3f, v1: vec3f, v2: vec3f) -> vec3f {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = cross(ray.direction, edge2);
    let det = dot(edge1, h);
    if det > -0.00001 && det < 0.00001 { // Backface culling: det < 0.00001
        return vec3f(NO_HIT, NO_HIT, NO_HIT); // Parallel
    }
    var inv_det = 1.0 / det;
    var s = ray.origin - v0;
    var u = inv_det * dot(s, h);
    if u < 0.0 || u > 1.0 {
        return vec3f(NO_HIT, NO_HIT, NO_HIT); // Outside
    }
    var q = cross(s, edge1);
    var v = inv_det * dot(ray.direction, q);
    if v < 0.0 || u + v > 1.0 {
        return vec3f(NO_HIT, NO_HIT, NO_HIT); // Outside
    }
    var t = inv_det * dot(edge2, q);
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
    return select(NO_HIT, t_near, t_near < t_far);
}

fn generate_ray(id: vec3u) -> Ray {
    let dim = vec2f(textureDimensions(output));
    let uv = 2.0 * vec2f(id.xy) / dim - 1.0;

    // let clip_pos = vec4f(0.0, 0.0, 0.0, 1.0);
    // let world_pos = camera.clip_to_world * clip_pos;
    let world_pos = camera.clip_to_world[3]; // Equivalent to the above
    let pos = world_pos.xyz / world_pos.w;

    let clip_dir = vec4f(-uv, -1.0, 1.0);
    let world_dir = camera.clip_to_world * clip_dir;
    let dir = pos - world_dir.xyz / world_dir.w;

    return Ray(
        pos,
        dir,
        1.0 / dir,
    );
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3u) {
    // var color = textureLoad(output, vec2i(id.xy));
    let ray = generate_ray(id);
    let t = intersect_AABB(ray, aabb) * 0.01;
    textureStore(output, id.xy, vec4f(t, t, t, 1.0));
}

const MAX_ITERATIONS: u32 = 1024u;

@compute
@workgroup_size(16, 16)
fn mandelbrot(@builtin(global_invocation_id) id: vec3u) {
    let dim = vec2f(textureDimensions(output));
    let uv = (2.0 * vec2f(id.xy) - dim) / dim.y;
    let c = uv * 1.2 - vec2f(0.7, 0.0); 
    var z = c;
    var i = 0u;
    for (; i < MAX_ITERATIONS; i++) {
        z = vec2f(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        if dot(z, z) > 16.0 {
            break;
        }
    }
    let value = f32(i) / f32(MAX_ITERATIONS);
    textureStore(output, id.xy, vec4f(value, value, value, 1.0));
}