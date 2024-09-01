struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) texcoord: vec2f,
};

@group(0) @binding(0)
var t: texture_2d<f32>;

@group(0) @binding(1)
var s: sampler;

fn fullscreen_triangle(i: u32) -> vec4f {
    switch (i) {
        case 0u, default: {
            return vec4f(-1.0, -1.0, 0.0, 1.0);
        }
        case 1u: {
            return vec4f(3.0, -1.0, 0.0, 1.0);
        }
        case 2u: {
            return vec4f(-1.0, 3.0, 0.0, 1.0);
        }
    }
}

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = fullscreen_triangle(in_vertex_index);
    out.texcoord = out.position.xy * 0.5 + 0.5;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return textureSample(t, s, in.texcoord);
}