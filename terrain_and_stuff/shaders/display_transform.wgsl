#import "srgb.wgsl"::srgb_from_linear

@group(0) @binding(0)
var hdr_backbuffer: texture_2d<f32>;

@fragment
fn fs_main(@location(0) texcoord: vec2<f32>) -> @location(0) vec4<f32> {
    let texture_dimensions = vec2f(textureDimensions(hdr_backbuffer).xy);
    let texel_coords = vec2u(texcoord * texture_dimensions);
    let hdr_backbuffer_color = textureLoad(hdr_backbuffer, texel_coords, 0);

    // TODO: actual display transform!

    return vec4f(srgb_from_linear(hdr_backbuffer_color.rgb), 1.0);
}
