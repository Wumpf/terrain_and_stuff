@group(1) @binding(0)
var shadowmap: texture_depth_2d;

@fragment
fn fs_main(@builtin(position) frag_coords: vec4<f32>) -> @location(0) vec4<f32> {

    let texel_coords = vec2u(frag_coords.xy);
    let shadow_distance = textureLoad(shadowmap, texel_coords, 0);

    if shadow_distance == 1.0 {
        return vec4f(0.0, 0.0, 1.0, 1.0);
    }

    let shadow_map_size = textureDimensions(shadowmap).xy;
    if texel_coords.x > shadow_map_size.x || texel_coords.y > shadow_map_size.y {
        return vec4f(1.0, 0.0, 1.0, 1.0);
    }

    return vec4f(shadow_distance);
}

