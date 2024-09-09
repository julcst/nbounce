const MAX_FLOAT: f32 = 0x1.fffffep+127f;
const NO_HIT: f32 = MAX_FLOAT;
const EPS: f32 = 0.00000001;
const STACK_SIZE = 32u;

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
                    if instance.emissive > 0.0 {
                        hit.flags |= EMISSIVE;
                    }
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
                // FIXME: This does interpolation for multiple triangles but it is only necessary for nearest
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