#import "constants.wgsl"::{ERROR_RGBA}
#import "global_bindings.wgsl"::{trilinear_sampler_clamp}
#import "srgb.wgsl"::srgb_oetf

@group(1) @binding(0)
var hdr_backbuffer: texture_2d<f32>;

@group(1) @binding(1)
var tony_mc_mapface_lut: texture_3d<f32>;


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

fn dither_interleaved(rgb: vec3f, levels: f32, frag_coord: vec2<f32>) -> vec3f {
    var noise = interleaved_gradient_noise(frag_coord);
    // scale down the noise slightly to ensure flat colors aren't getting dithered
    noise = (noise - 0.5) * 0.95;
    return rgb + noise / (levels - 1.0);
}

// See https://github.com/h3r2tic/tony-mc-mapface/blob/0f249d366c9e960aa9828818786a6d5900fd85d9/shader/tony_mc_mapface.hlsl
fn tony_mc_mapface(stimulus: vec3f) -> vec3f {
    // Apply a non-linear transform that the LUT is encoded with.
    let encoded = stimulus / (stimulus + 1.0);

    // Align the encoded range to texel centers.
    let LUT_DIMS = 48.0;
    let uv = encoded * ((LUT_DIMS - 1.0) / LUT_DIMS) + 0.5 / LUT_DIMS;

    return textureSampleLevel(tony_mc_mapface_lut, trilinear_sampler_clamp, uv, 0.0).rgb;
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

    // Debug: Demonstrate how a bright single channel color moves towards white.
    //hdr_backbuffer_color = vec4f(10.0, 0.0,0.0, 1.0);

    // Apply display transform!
    let display_transformed = tony_mc_mapface(hdr_backbuffer_color.rgb);
    // Debug: Display transform by just clamping the stimulus.
    //let display_transformed = saturate(hdr_backbuffer_color.rgb);

    // Apply EOTF.
    let output_signal = srgb_oetf(display_transformed);
    // Apply dithering in output signal space.
    let dithered_output_signal = dither_interleaved(output_signal, 256.0, frag_coords);

    return vec4f(dithered_output_signal, 1.0);


    // Debug: show a slice of the display transform LUT.
    //return textureSampleLevel(tony_mc_mapface_lut, trilinear_sampler_clamp, vec3f(texcoord, 0.8), 0.0);
}

