// Multiple scattering LUT.
//
// Each pixel coordinate corresponds to a sun zenith angle (x axis) and height (y axis).
// The value is the isotropic multiple scattering contribution to the overall luminance.

#import "constants.wgsl"::{ERROR_RGBA, TAU}

const MultipleScatteringSteps: f32 = 20.0;
const MultipleScatteringSamplesSqrt: u32 = 8; // 64 directional samples.

@fragment
fn fs_main(@location(0) texcoord: vec2f) -> @location(0) vec4<f32> {
    // TODO:
    return vec4f(0.0, 0.0, 0.0, 1.0);
}
