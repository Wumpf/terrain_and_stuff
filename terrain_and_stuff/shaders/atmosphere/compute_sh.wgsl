//! Compute shader for computing luminace SH coefficients for the atmosphere at a fixed altitude.

#import "global_bindings.wgsl"::{frame_uniforms}
#import "constants.wgsl"::{TAU}
#import "sh.wgsl"::{
    sh_weight_00,
    sh_weight_1n1,
    sh_weight_10,
    sh_weight_1p1,
    sh_weight_2n2,
    sh_weight_2n1,
    sh_weight_20,
    sh_weight_2p1,
    sh_weight_2p2,
}

#import "atmosphere/params.wgsl"::{atmosphere_params}
#import "atmosphere/raymarch.wgsl"::{raymarch_scattering}
#import "atmosphere/sky_and_sun_lighting.wgsl"::{SkyAndSunLightingParams}

// naga_oil doesn't support override constants :(
//override NUM_SAMPLES: u32;
const NUM_SAMPLES: u32 = 1024;
const SAMPLE_NORMALIZATION_FACTOR: f32 = (1.0 / f32(NUM_SAMPLES)) * (2.0 * TAU); // Normalize by the number of samples and the sphere's surface area.

@group(2) @binding(0) var transmittance_lut: texture_2d<f32>;
@group(2) @binding(1) var multiple_scattering_lut: texture_2d<f32>;
@group(2) @binding(2) var<storage, read> sampling_directions: array<vec3f>;
@group(2) @binding(3) var<storage, read_write> sky_and_sun_lighting_params: SkyAndSunLightingParams;

var<workgroup> shared_buffer: array<vec3f, NUM_SAMPLES>;

fn parallel_reduce_shared_buffer(sample: vec3f, sample_index: u32, target_coefficient_index: u32) {
    shared_buffer[sample_index] = sample;
    workgroupBarrier();

    // In an optimal implementation, we'd do special handling once we're down to the size of the subgroup (aka warp).
    // Practically, subgroup sizes depend too much on GPU details.
    for (var i = NUM_SAMPLES / 2; i >= 1; i /= 2) {
        if (sample_index < i) {
            shared_buffer[sample_index] += shared_buffer[sample_index + i];
        }
        workgroupBarrier();
    }

    if (sample_index == 0) {
        sky_and_sun_lighting_params.
            sky_luminance_sh_coefficients[target_coefficient_index] = shared_buffer[0] * SAMPLE_NORMALIZATION_FACTOR;
    }
}

@compute @workgroup_size(NUM_SAMPLES) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let sample_index = id.x;
    // TODO: Just learned about Fibbonaci lattice
    // https://extremelearning.com.au/how-to-evenly-distribute-points-on-a-sphere-more-effectively-than-the-canonical-fibonacci-lattice/
    // Using this over on `lut_multiple_scattering.wgsl`. Should use that here as well.
    // Throw out halton sequence stuff again.
    let direction = sampling_directions[sample_index];

    let planet_relative_position_km = vec3f(0.0, atmosphere_params.ground_radius_km + 0.2, 0.0); // Put the SH "probe" at 200m altitude.
    let max_marching_distance_km = 999999999999.0;

    var sample_raymarch_result = raymarch_scattering(
        transmittance_lut,
        multiple_scattering_lut,
        direction,
        planet_relative_position_km,
        frame_uniforms.dir_to_sun,
        max_marching_distance_km
    );

    // We're interested in the sky color, i.e. all the light that got scattered in.
    let luminance_sample = sample_raymarch_result.scattering;

    parallel_reduce_shared_buffer(luminance_sample * sh_weight_00(direction), sample_index, 0);

    parallel_reduce_shared_buffer(luminance_sample * sh_weight_1n1(direction), sample_index, 1);
    parallel_reduce_shared_buffer(luminance_sample * sh_weight_10(direction), sample_index, 2);
    parallel_reduce_shared_buffer(luminance_sample * sh_weight_1p1(direction), sample_index, 3);

    parallel_reduce_shared_buffer(luminance_sample * sh_weight_2n2(direction), sample_index, 4);
    parallel_reduce_shared_buffer(luminance_sample * sh_weight_2n1(direction), sample_index, 5);
    parallel_reduce_shared_buffer(luminance_sample * sh_weight_20(direction), sample_index, 6);
    parallel_reduce_shared_buffer(luminance_sample * sh_weight_2p1(direction), sample_index, 7);
    parallel_reduce_shared_buffer(luminance_sample * sh_weight_2p2(direction), sample_index, 8);

    // TODO? SH windowing to avoid negatives & ringing?

    // Compute sun luminance.
    if (sample_index == 0) {
        let sun_raymarch_result = raymarch_scattering(
            transmittance_lut,
            multiple_scattering_lut,
            frame_uniforms.dir_to_sun,
            planet_relative_position_km,
            frame_uniforms.dir_to_sun,
            max_marching_distance_km
        );

        // For Sky color we're in-scattered light, but when we're looking directly into the sun,
        // what we're seeing is the sun light itself transmitted through the atmosphere, which is why we have to use transmittance here.
        sky_and_sun_lighting_params.sun_illuminance = sun_raymarch_result.transmittance * atmosphere_params.sun_illuminance;
    }
}
