struct VertexOutput {
    @builtin(position) clip_position: vec4f,
};

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
    out.clip_position = fullscreen_triangle(in_vertex_index);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return vec4f(0.3, 0.2, 0.1, 1.0);
}