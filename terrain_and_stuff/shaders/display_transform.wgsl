#import "constants.wgsl"::{ERROR_RGBA}
#import "srgb.wgsl"::srgb_from_linear

@group(0) @binding(0)
var hdr_backbuffer: texture_2d<f32>;

@fragment
fn fs_main(@location(0) texcoord: vec2<f32>) -> @location(0) vec4<f32> {
    let texture_dimensions = vec2f(textureDimensions(hdr_backbuffer).xy);
    let texel_coords = vec2u(texcoord * texture_dimensions);
    var hdr_backbuffer_color = textureLoad(hdr_backbuffer, texel_coords, 0);

    // NaN detector.
    if hdr_backbuffer_color.r != hdr_backbuffer_color.r ||
        hdr_backbuffer_color.g != hdr_backbuffer_color.g ||
        hdr_backbuffer_color.b != hdr_backbuffer_color.b {
        return ERROR_RGBA;
    }

    // TODO: actual display transform!
    hdr_backbuffer_color = saturate(hdr_backbuffer_color);

    return vec4f(srgb_from_linear(hdr_backbuffer_color.rgb), 1.0);
}
