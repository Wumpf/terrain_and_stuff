#import "constants.wgsl"::{ERROR_RGBA}
#import "srgb.wgsl"::srgb_from_linear

@group(0) @binding(0)
var hdr_backbuffer: texture_2d<f32>;

// Adapted from
// https://www.shadertoy.com/view/llVGzG
// Originally presented in:
// Jimenez 2014, "Next Generation Post-Processing in Call of Duty"
//
// A good overview can be found in
// https://blog.demofox.org/2022/01/01/interleaved-gradient-noise-a-different-kind-of-low-discrepancy-sequence/
// via https://github.com/rerun-io/rerun/
fn interleaved_gradient_noise(n: vec2<f32>) -> f32 {
    let f = 0.06711056 * n.x + 0.00583715 * n.y;
    return fract(52.9829189 * fract(f));
}

fn dither_interleaved(rgb: vec3<f32>, levels: f32, frag_coord: vec2<f32>) -> vec3<f32> {
    var noise = interleaved_gradient_noise(frag_coord);
    // scale down the noise slightly to ensure flat colors aren't getting dithered
    noise = (noise - 0.5) * 0.95;
    return rgb + noise / (levels - 1.0);
}

@fragment
fn fs_main(@location(0) texcoord: vec2<f32>) -> @location(0) vec4<f32> {
    let texture_dimensions = vec2f(textureDimensions(hdr_backbuffer).xy);
    let frag_coords = texcoord * texture_dimensions;
    let texel_coords = vec2u(frag_coords);
    var hdr_backbuffer_color = textureLoad(hdr_backbuffer, texel_coords, 0);

    // NaN detector.
    if hdr_backbuffer_color.r != hdr_backbuffer_color.r ||
        hdr_backbuffer_color.g != hdr_backbuffer_color.g ||
        hdr_backbuffer_color.b != hdr_backbuffer_color.b {
        return ERROR_RGBA;
    }

    // TODO: expose exposure factor
    let exposure_factor = 1.0;
    hdr_backbuffer_color = hdr_backbuffer_color * exposure_factor;

    // TODO: actual display transform!
    hdr_backbuffer_color = saturate(hdr_backbuffer_color);

    // Apply EOTF.
    let output_signal = srgb_from_linear(hdr_backbuffer_color.rgb);
    // Apply dithering in output signal space.
    let dithered_output_signal = dither_interleaved(output_signal, 256.0, frag_coords);

    return vec4f(dithered_output_signal, 1.0);
}
