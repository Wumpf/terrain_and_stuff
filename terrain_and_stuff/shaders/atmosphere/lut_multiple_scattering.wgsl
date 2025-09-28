// Multiple scattering LUT.
//
// Each pixel coordinate corresponds to a sun zenith angle from 0 to pi (x axis) and height (y axis).
// The value is the isotropic multiple scattering contribution to the overall luminance.
// It is a transfer function transferring illuminance to its multiple scattering contribution as luminance.

#import "constants.wgsl"::{ERROR_RGBA, PI}
#import "intersections.wgsl"::{ray_sphere_intersect, Ray}
#import "atmosphere/params.wgsl"::{atmosphere_params}
#import "atmosphere/scattering.wgsl"::{scattering_values_for, mie_phase, rayleigh_phase, sample_transmittance_lut}
#import "sampling.wgsl"::{uniform_sampled_sphere_direction}

const DirectionSampleCount: u32 = 256;
const MultipleScatteringSteps: u32 = 20;

@group(2) @binding(0) var lut_transmittance: texture_2d<f32>;

fn ray_to_sun_texcoord(texcoord: vec2f) -> Ray {
    let sun_cos_theta = 2.0 * texcoord.x - 1.0;
    let sun_theta = acos(sun_cos_theta);
    let height_km = mix(atmosphere_params.ground_radius_km, atmosphere_params.atmosphere_radius_km, texcoord.y);
    let planet_relative_pos = vec3f(0.0, height_km, 0.0);
    let dir_to_sun = vec3f(0.0, sun_cos_theta, -sin(sun_theta));

    return Ray(planet_relative_pos, dir_to_sun);
}

fn spherical_dir(phi: f32, theta: f32) -> vec3f {
    let cosPhi = cos(phi);
    let sinPhi = sin(phi);
    let cosTheta = cos(theta);
    let sinTheta = sin(theta);
    return vec3f(sinPhi*sinTheta, cosPhi, sinPhi*cosTheta);
}

@fragment
fn fs_main(@location(0) texcoord: vec2f) -> @location(0) vec4<f32> {
    let ray_to_sun_km = ray_to_sun_texcoord(texcoord);

    // Inscattering contribution of second order, using a bunch of simplifying assumptions.
    var second_order_scattering = vec3f(0.0);
    // "infinite multiple scattering light contribution factor F_ms as a geometric series infinite sum"
    // I.e. an approximation to get from second order scattering to infinite order scattering.
    var multiple_scattering_factor = vec3f(0.0);

    // Iterate over sphere samples.
    for (var direction_index: u32 = 0; direction_index < DirectionSampleCount; direction_index += 1) {
        let sample_dir = uniform_sampled_sphere_direction(direction_index, DirectionSampleCount);
        let sample_ray_planet_km = Ray(ray_to_sun_km.origin, sample_dir);

        let atmosphere_distance_km = ray_sphere_intersect(sample_ray_planet_km, atmosphere_params.atmosphere_radius_km);
        let ground_distance_km = ray_sphere_intersect(sample_ray_planet_km, atmosphere_params.ground_radius_km);
        let max_marching_distance_km = select(ground_distance_km, atmosphere_distance_km, ground_distance_km < 0.0);

        // Evaluate phase functions along the marching direction.
        let cos_theta = dot(sample_ray_planet_km.direction, ray_to_sun_km.direction);
        let rayleigh_phase_value = rayleigh_phase(cos_theta);
        let mie_phase_value = mie_phase(cos_theta);

        var transmittance = vec3f(1.0);
        let dt = max_marching_distance_km / f32(MultipleScatteringSteps);
        var t = 0.0;
        const sample_segment_t: f32 = 0.3;

        // Loop similar to raymarch.wgsl#raymarch_scattering
        // But we're not marching towards the sun, but along the sample direction.
        for (var i: u32 = 0; i < MultipleScatteringSteps; i += 1) {
            let t_new = (f32(i) + sample_segment_t) * dt;
            let dt_exact = t_new - t;
            t = t_new;

            let new_planet_relative_position_km = sample_ray_planet_km.origin + t * sample_ray_planet_km.direction;
            let sample_height = length(new_planet_relative_position_km);
            let altitude_km = max(0.0, sample_height - atmosphere_params.ground_radius_km);
            let zenith = new_planet_relative_position_km / sample_height;
            let sun_cos_zenith_angle = dot(zenith, ray_to_sun_km.direction);

            let scattering = scattering_values_for(altitude_km);
            let sample_transmittance = exp(-dt_exact * scattering.total_extinction_per_km);

            // Integrate within each segment.
            // Simplifying scattering by assuming it to be isotropic.
            let scattering_no_phase = scattering.rayleigh + scattering.mie;
            let scattering_f = (scattering_no_phase - scattering_no_phase * sample_transmittance) / scattering.total_extinction_per_km;
            multiple_scattering_factor += transmittance * scattering_f;

            let sun_transmittance = sample_transmittance_lut(lut_transmittance, altitude_km, sun_cos_zenith_angle);

            let rayleigh_in_scattering = scattering.rayleigh * rayleigh_phase_value;
            let mie_in_scattering = scattering.mie * mie_phase_value;
            let total_in_scattering = (rayleigh_in_scattering + mie_in_scattering) * sun_transmittance;

            // Integrated scattering within path segment.
            let scattering_integral = (total_in_scattering - total_in_scattering * sample_transmittance) / scattering.total_extinction_per_km;

            second_order_scattering += scattering_integral * transmittance;
            transmittance *= sample_transmittance;
        }

        // If we hit the ground, add the ground albedo to the luminance.
        if ground_distance_km > 0.0 {
            let new_planet_relative_position_km = sample_ray_planet_km.origin + max_marching_distance_km * sample_ray_planet_km.direction;
            let altitude_km = 0.0;
            let zenith = normalize(new_planet_relative_position_km);
            let sun_cos_zenith_angle = dot(zenith, ray_to_sun_km.direction);

            let sun_transmittance_at_ground = sample_transmittance_lut(lut_transmittance, altitude_km, sun_cos_zenith_angle);
            second_order_scattering += transmittance * atmosphere_params.ground_albedo * sun_transmittance_at_ground * saturate(sun_cos_zenith_angle) / PI;
        }
    }

    multiple_scattering_factor /= f32(DirectionSampleCount);
    second_order_scattering /= f32(DirectionSampleCount);

    // (Equations (10) and (9) from the paper)
    let multiple_scattering = second_order_scattering * (1.0 - multiple_scattering_factor);

    return vec4f(multiple_scattering, 1.0);
}
