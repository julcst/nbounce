const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 2.0 * PI;
const INV_PI: f32 = 1.0 / PI;
const INV_TWO_PI: f32 = 1.0 / TWO_PI;

/// Maps a random u32 to a random float in [0,1), see https://blog.bithole.dev/blogposts/random-float/
fn map4f(x: vec4u) -> vec4f {
    return bitcast<vec4f>((x >> vec4u(9u)) | vec4u(0x3F800000u)) - 1.0;
}

/// Maps a random u32 to a random float in [0,1), see https://blog.bithole.dev/blogposts/random-float/
fn map2f(x: vec2u) -> vec2f {
    return bitcast<vec2f>((x >> vec2u(9u)) | vec2u(0x3F800000u)) - 1.0;
}

/// PCG2D from http://jcgt.org/published/0009/03/02/
fn hash2u(s: vec2u) -> vec2u {
    var v = s * 1664525u + 1013904223u;
    v.x += v.y * 1664525u; v.y += v.x * 1664525u;
    v ^= v >> vec2u(16u);
    v.x += v.y * 1664525u; v.y += v.x * 1664525u;
    v ^= v >> vec2u(16u);
    return v;
}

/// PCG4D from http://jcgt.org/published/0009/03/02/
fn hash4u(s: vec4u) -> vec4u {
    var v = s * 1664525u + 1013904223u;
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
    v ^= v >> vec4u(16u);
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
    return v;
}

fn hash2f(s: vec2u) -> vec2f {
    return map2f(hash2u(s));
}

fn hash4f(s: vec4u) -> vec4f {
    return map4f(hash4u(s));
}

/// Perceived luminance of a linear color (https://en.wikipedia.org/wiki/Relative_luminance)
fn luminance(linear_rgb: vec3f) -> f32 {
    return dot(vec3f(0.2126, 0.7152, 0.0722), linear_rgb);
}

/// Returns the upper left 3x3 submatrix of a 4x4 matrix
fn mat3(m: mat4x4f) -> mat3x3f {
    return mat3x3f(m[0].xyz, m[1].xyz, m[2].xyz);
}