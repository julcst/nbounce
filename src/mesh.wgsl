struct CameraData {
    world_to_view: mat4x4f,
    view_to_world: mat4x4f,
    view_to_clip: mat4x4f,
    world_to_clip: mat4x4f,
};

@group(0) @binding(0) var<uniform> camera: CameraData;

struct VertexInput {
    @location(0) position: vec4f,
    @location(1) normal: vec4f,
    @location(2) texcoord: vec4f,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) normal: vec4f,
    @location(1) texcoord: vec4f,
};

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.world_to_clip * model.position;
    out.normal = model.normal;
    out.texcoord = model.texcoord;
    return out;
}

fn checkerboard(texcoord: vec4f) -> vec4f {
    let c = floor(texcoord.xy * 8.0);
    let color = (c.x + c.y) % 2.0;
    return vec4f(color, color, color, 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return checkerboard(in.texcoord);
}
