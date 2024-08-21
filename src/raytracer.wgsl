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

struct Vertex {
    position: vec4f,
    normal: vec4f,
    texcoord: vec4f,
};

struct BVHNode {
    min: vec3f,
    start: u32,
    max: vec3f,
    count: u32,
};

@group(2)
@binding(0)
var<storage, read> bvh: array<BVHNode>;

@group(2)
@binding(1)
var<storage, read> vertices: array<Vertex>;

@group(2)
@binding(2)
var<storage, read> indices: array<u32>;

const COMPUTE_SIZE: u32 = 8u;
const PI: f32 = 3.14159265359;
const MAX_FLOAT: f32 = 0x1.fffffep+127f;
const NO_HIT: f32 = MAX_FLOAT;

struct Ray {
    origin: vec3f,
    direction: vec3f,
    inv_direction: vec3f,
};

struct AABB {
    min: vec3f,
    max: vec3f,
};

const EPS: f32 = 0.00000001;

// Möller–Trumbore intersection algorithm
fn intersect_triangle(ray: Ray, v0: vec3f, v1: vec3f, v2: vec3f) -> vec3f {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = cross(ray.direction, edge2);
    let det = dot(edge1, h);
    if det > -EPS && det < EPS { // Backface culling: det < EPS
        return vec3f(NO_HIT, NO_HIT, NO_HIT); // Parallel
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
    if t < 0 {
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

struct HitInfo {
    dist: f32,
    position: vec3f,
    normal: vec3f,
    texcoord: vec2f,
    n_aabb: u32,
    n_tri: u32,
};

fn intersect_BVH(ray: Ray) -> HitInfo {
    var stack: array<u32, 32>;

    var hit = HitInfo(NO_HIT, vec3f(0.0), vec3f(0.0), vec2f(0.0), 0u, 0u);

    // Init stack with top node
    var i = 0u;

    let index_top = 0u;
    let top = bvh[index_top];
    let dist_top = intersect_AABB(ray, AABB(top.min, top.max));
    hit.n_aabb += 1u;
    if dist_top < hit.dist {
        stack[i] = index_top;
        i += 1u;
    }

    while i > 0u {
        // Pop next node from stack
        i -= 1u;
        var node = bvh[stack[i]];
        let is_leaf = node.count > 0u;
        if is_leaf { // Leaf node
            for (var j = node.start * 3u; j < (node.start + node.count) * 3u; j += 3u) {
                let v0 = vertices[indices[j + 0u]];
                let v1 = vertices[indices[j + 1u]];
                let v2 = vertices[indices[j + 2u]];
                let t = intersect_triangle(ray, v0.position.xyz, v1.position.xyz, v2.position.xyz);
                hit.n_tri += 1u;
                if t.x < hit.dist {
                    hit.dist = t.x;
                    let barycentrics = vec3f(1.0 - t.y - t.z, t.yz);
                    hit.position = ray.origin + hit.dist * ray.direction;
                    hit.normal = normalize(mat3x3f(v0.normal.xyz, v1.normal.xyz, v2.normal.xyz) * barycentrics);
                    hit.texcoord = mat3x2f(v0.texcoord.xy, v1.texcoord.xy, v2.texcoord.xy) * barycentrics;
                }
            }
        } else {
            let index_left = node.start;
            let left = bvh[index_left];
            let dist_left = intersect_AABB(ray, AABB(left.min, left.max));

            let index_right = index_left + 1u;
            let right = bvh[index_right];
            let dist_right = intersect_AABB(ray, AABB(right.min, right.max));
            hit.n_aabb += 2u;

            let is_left_nearest = dist_left < dist_right;

            let dist_far = select(dist_left, dist_right, is_left_nearest);
            let index_far = select(index_left, index_right, is_left_nearest);
            let dist_near = select(dist_right, dist_left, is_left_nearest);
            let index_near = select(index_right, index_left, is_left_nearest);

            if dist_far < hit.dist {
                stack[i] = index_far;
                i += 1u;
            }

            if dist_near < hit.dist {
                stack[i] = index_near;
                i += 1u;
            }
        }
    }
    return hit;
}

// TODO: Match with rasterization
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

    return Ray(pos, dir, 1.0 / dir);
}

@compute
@workgroup_size(COMPUTE_SIZE, COMPUTE_SIZE)
fn main(@builtin(global_invocation_id) id: vec3u) {
    // var color = textureLoad(output, vec2i(id.xy));
    let ray = generate_ray(id);
    let hit = intersect_BVH(ray);
    let is_hit = (hit.dist != NO_HIT);
    textureStore(output, id.xy, vec4f(f32(hit.n_aabb) * 0.05, select(0.0, 1.0, is_hit), f32(hit.n_tri) * 0.1, 1.0));
    //textureStore(output, id.xy, vec4f(hit.normal * 0.5 + 0.5, 1.0));
    //textureStore(output, id.xy, vec4f(hit.texcoord, 0.0, 1.0));
}