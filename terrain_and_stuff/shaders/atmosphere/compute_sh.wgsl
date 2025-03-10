//! Compute shader for computing luminace SH coefficients for the atmosphere at a fixed altitude.

#import "global_bindings.wgsl"::{frame_uniforms}
#import "atmosphere/constants.wgsl"::{ground_radius_km}
#import "atmosphere/raymarch.wgsl"::{raymarch_scattering}

// naga_oil doesn't support override constants :(
//override NUM_SAMPLES: u32;
const NUM_SAMPLES: u32 = 1024;

@group(1) @binding(0) var transmittance_lut: texture_2d<f32>;
@group(1) @binding(1) var<storage, read> sampling_directions: array<vec3f>;
@group(1) @binding(2) var<storage, read_write> sh_coefficients: array<vec3f>;

@compute @workgroup_size(NUM_SAMPLES) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let sample_index = id.x;
    let direction = sampling_directions[sample_index];
    let planet_relative_position_km = vec3f(0.0, ground_radius_km + 0.5, 0.0); // Put the SH "probe" at 500m altitude.
    let max_marching_distance_km = 999999999999.0;

    var luminance_sample = raymarch_scattering(
        transmittance_lut,
        direction,
        planet_relative_position_km,
        frame_uniforms.dir_to_sun,
        max_marching_distance_km
    ).scattering;

    // TODO: do prefix sum for each SH coefficient for this sample.

    sh_coefficients[0] = luminance_sample;
    sh_coefficients[1] = sampling_directions[1];
    sh_coefficients[2] = vec3f(6.0, 7.0, 8.0);
    sh_coefficients[3] = vec3f(9.0, 10.0, 11.0);
    sh_coefficients[4] = vec3f(12.0, 13.0, 14.0);
    sh_coefficients[5] = vec3f(15.0, 16.0, 17.0);
    sh_coefficients[6] = vec3f(18.0, 19.0, 20.0);
}
