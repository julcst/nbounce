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

struct BVHNode {
    min: vec3f,
    start: u32,
    max: vec3f,
    end: u32,
};

@group(2)
@binding(0)
var<storage, read> blas: array<BVHNode>;

@group(2)
@binding(1)
var<storage, read> tlas: array<BVHNode>;

struct Instance {
    world_to_local: mat4x4f,
    color: vec4f,
    roughness: f32,
    metallic: f32,
    emissive: f32,
    node: u32,
};

@group(2)
@binding(2)
var<storage, read> instances: array<Instance>;

struct Vertex {
    position: vec4f,
    normal: vec4f,
    texcoord: vec4f,
};

@group(2)
@binding(3)
var<storage, read> vertices: array<Vertex>;

@group(2)
@binding(4)
var<storage, read> indices: array<u32>;

@group(2)
@binding(5)
var environment: texture_cube<f32>;

@group(2)
@binding(6)
var environment_sampler: sampler;

const COMPUTE_SIZE: u32 = 8u;
const PI: f32 = 3.14159265359;
const MAX_FLOAT: f32 = 0x1.fffffep+127f;
const NO_HIT: f32 = MAX_FLOAT;
const GAIN: f32 = 0.25;
const EPS: f32 = 0.00000001;

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
    position: vec3f,
    dist: f32,
    normal: vec3f,
    n_aabb: u32,
    texcoord: vec2f,
    n_tri: u32,
    roughness: f32,
    color: vec4f,
    metallic: f32,
};

fn no_hit_info() -> HitInfo {
    return HitInfo(vec3f(0.0), NO_HIT, vec3f(0.0), 0u, vec2f(0.0), 0u, 0.0, vec4f(0.0), 0.0);
}

struct StackEntry {
    index: u32,
    dist: f32,
};

fn intersect_TLAS(ray: Ray) -> HitInfo {
    var stack: array<StackEntry, 32>;

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
                let local_origin = instances[j].world_to_local * vec4f(ray.origin, 1.0);
                let local_direction = instances[j].world_to_local * vec4f(ray.direction, 0.0);
                let local_ray = Ray(local_origin.xyz, local_direction.xyz, 1.0 / local_direction.xyz);
                let hit_local = intersect_BLAS(local_ray, instance.node);
                hit.n_aabb += hit_local.n_aabb;
                hit.n_tri += hit_local.n_tri;
                if hit_local.dist < hit.dist {
                    hit.dist = hit_local.dist;
                    hit.position = ray.origin + hit.dist * ray.direction;
                    hit.normal = normalize((instance.world_to_local * vec4f(hit_local.normal, 0.0)).xyz);
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
        hit.color = textureSampleLevel(environment, environment_sampler, ray.direction, 0.0);
    }
    return hit;
}

fn intersect_BLAS(ray: Ray, index_top: u32) -> HitInfo {
    var stack: array<StackEntry, 32>;

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

fn intersect_Instances(ray: Ray) -> HitInfo {
    var hit = no_hit_info();
    for (var j = 0u; j < arrayLength(&instances); j += 1u) {
        let instance = instances[j];
        let local_origin = instances[j].world_to_local * vec4f(ray.origin, 1.0);
        let local_direction = instances[j].world_to_local * vec4f(ray.direction, 0.0);
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

@compute
@workgroup_size(COMPUTE_SIZE, COMPUTE_SIZE)
fn main(@builtin(global_invocation_id) id: vec3u) {
    // var color = textureLoad(output, vec2i(id.xy));
    let ray = generate_ray(id);

    let hit = intersect_TLAS(ray);
    //let hit = intersect_Instances(ray);
    //let hit = intersect_BLAS(ray, 30903u);
    //let hit = intersect_BLAS(ray, 0u);
    
    let is_hit = (hit.dist != NO_HIT);
    //textureStore(output, id.xy, vec4f(f32(hit.n_aabb) * 0.02, select(0.0, 1.0, is_hit), f32(hit.n_tri) * 0.2, 1.0));
    textureStore(output, id.xy, hit.color * GAIN);
    //textureStore(output, id.xy, vec4f(vec3f(hit.roughness), 1.0));
    //textureStore(output, id.xy, vec4f(vec3f(hit.metallic), 1.0));
    //textureStore(output, id.xy, vec4f(hit.normal * 0.5 + 0.5, 1.0));
    //textureStore(output, id.xy, vec4f(hit.texcoord, 0.0, 1.0));
    //textureStore(output, id.xy, vec4f(hit.position, 1.0));
}