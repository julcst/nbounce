struct CameraData {
    world_to_clip: mat4x4f,
    clip_to_world: mat4x4f,
};

@group(0) @binding(0) var<uniform> camera: CameraData;

struct VertexInput {
    @location(0) position: vec3f,
    @location(1) u: f32,
    @location(2) normal: vec3f,
    @location(3) v: f32,
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
    out.clip_position = camera.world_to_clip * vec4f(model.position, 1.0);
    out.normal = vec4f(model.normal, 0.0);
    out.texcoord = vec4f(model.u, model.v, 0.0, 0.0);
    return out;
}

fn checkerboard(texcoord: vec4f) -> vec4f {
    let c = floor(texcoord.xy * 15.0);
    let color = (c.x + c.y) % 2.0 * 0.5 + 0.25;
    return vec4f(color, color, color, 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return checkerboard(in.texcoord);
}
