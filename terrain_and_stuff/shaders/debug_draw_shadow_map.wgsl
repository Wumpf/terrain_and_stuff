@group(1) @binding(0)
var shadowmap: texture_2d<f32>;

@fragment
fn fs_main(@builtin(position) frag_coords: vec4<f32>) -> @location(0) vec4<f32> {
    let texel_coords = vec2u(frag_coords.xy);
    var shadow_distance = textureLoad(shadowmap, texel_coords, 0).x;

    return vec4f(shadow_distance,shadow_distance,shadow_distance, 1.0);
}

