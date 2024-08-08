@group(0)
@binding(0)
var texture: texture_storage_2d<rgba16float, read_write>;

@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3u) {
    var color = textureLoad(texture, vec2i(id.xy));
    color.r += 0.01;
    textureStore(texture, vec2i(id.xy), color);
}