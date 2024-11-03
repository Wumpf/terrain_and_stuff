// Raymarch the sky color using transmittance & multiple scattering luts.
//
// This is what gives us the final sky color and light scattering overlay.
//
// The original technique describes also how to put this into a lat-long lookup texture.
// Doing so is a lot more efficient and screen resolution decoupled!
// However, this makes any interaction with occluders an approximation.
// The "Aerial Perspective LUT" (a volume texture for storing luminance & transmittance) use for this
// in the paper is a very good approximation, but naturally quite reach the quality of full per-pixel raymarching.

#import "global_bindings.wgsl"::trilinear_sampler

@group(1) @binding(0)
var transmittance_lut: texture_2d<f32>;

@fragment
fn fs_main(@location(0) texcoord: vec2f) -> @location(0) vec4<f32> {
    return textureSample(transmittance_lut, trilinear_sampler, texcoord);

    // TODO: Stuff!
    //return vec4<f32>(1.0, 0.0, 1.0, 1.0);
}
