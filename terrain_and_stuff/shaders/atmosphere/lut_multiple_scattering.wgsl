// Multiple scattering LUT.
//
// Each pixel coordinate corresponds to a sun zenith angle from 0 to pi (x axis) and height (y axis).
// The value is the isotropic multiple scattering contribution to the overall luminance.
// It is a transfer function transferring illuminance to its multiple scattering contribution as luminance.

#import "constants.wgsl"::{ERROR_RGBA, PI, GOLDEN_RATIO}
#import "intersections.wgsl"::{ray_sphere_intersect, Ray}
#import "atmosphere/params.wgsl"::{atmosphere_params}
#import "atmosphere/scattering.wgsl"::{scattering_values_for, mie_phase, rayleigh_phase, sample_transmittance_lut}

const MultipleScatteringSteps: f32 = 20.0;
const MultipleScatteringSamplesSqrt: f32 = 8.0; // 64 directional samples.
const DirectionSampleCount: u32 = 64;

@group(2) @binding(0) var lut_transmittance: texture_2d<f32>;

fn ray_to_sun_texcoord(texcoord: vec2f) -> Ray {
    let sun_cos_theta = 2.0 * texcoord.x - 1.0;
    let sun_theta = acos(sun_cos_theta);
    let height_km = mix(atmosphere_params.ground_radius_km, atmosphere_params.atmosphere_radius_km,
                        texcoord.y);
    let planet_relative_pos = vec3f(0.0, height_km, 0.0);
    let dir_to_sun = normalize(vec3f(0.0, sun_cos_theta, -sin(sun_theta)));

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
        // This integral is symmetric about theta = 0 (or theta = PI), so we
        // only need to integrate from zero to PI, not zero to TAU.
        // As for the rest: Fibboanci lattice for point sampling on sphere is just magic!
        // https://extremelearning.com.au/how-to-evenly-distribute-points-on-a-sphere-more-effectively-than-the-canonical-fibonacci-lattice/
        let direction_index_f = f32(direction_index);
        let theta = PI * direction_index_f / GOLDEN_RATIO;
        let phi = acos(1.0 - 2.0 * (direction_index_f + 0.5) / f32(DirectionSampleCount));

        let sample_dir = spherical_dir(phi, theta);
        let sample_ray_planet_km = Ray(ray_to_sun_km.origin, sample_dir);


        let atmosphere_distance_km = ray_sphere_intersect(sample_ray_planet_km, atmosphere_params.atmosphere_radius_km);
        let ground_distance_km = ray_sphere_intersect(sample_ray_planet_km, atmosphere_params.ground_radius_km);
        let max_marching_distance_km = select(ground_distance_km, atmosphere_distance_km, ground_distance_km < 0.0);

        // Evaluate phase functions along the marching direction.
        let cos_theta = dot(sample_ray_planet_km.direction, ray_to_sun_km.direction);
        let rayleigh_phase_value = rayleigh_phase(cos_theta);
        let mie_phase_value = mie_phase(cos_theta);

        var transmittance = vec3f(1.0);
        var t = 0.0;
        const sample_segment_t: f32 = 0.3;

        // Loop similar to raymarch.wgsl#raymarch_scattering
        // But we're not marching towards the sun, but along the sample direction.
        for (var i = 0.0; i < MultipleScatteringSteps; i += 1.0) {
            let t_new = ((i + sample_segment_t) / MultipleScatteringSteps) * max_marching_distance_km;
            let dt = t_new - t;
            t = t_new;

            let new_planet_relative_position_km = sample_ray_planet_km.origin + t * sample_ray_planet_km.direction;
            let altitude_km = clamp(length(new_planet_relative_position_km) - atmosphere_params.ground_radius_km,
                                    0.0, atmosphere_params.atmosphere_radius_km);

            let scattering = scattering_values_for(altitude_km);
            let sample_transmittance = exp(-dt * scattering.total_extinction);

            // Integrate within each segment.
            // Simplifying scattering by assuming it to be isotropic.
            let scattering_no_phase = scattering.rayleigh + scattering.mie;
            let scattering_f = (scattering_no_phase - scattering_no_phase * sample_transmittance) / scattering.total_extinction;
            multiple_scattering_factor += transmittance * scattering_f;

            // TODO: ShaderToy says:
            // This is slightly different from the paper, but I think the paper has a mistake?
            // In equation (6), I think S(x,w_s) should be S(x-tv,w_s).
            let sun_transmittance = sample_transmittance_lut(lut_transmittance, altitude_km, ray_to_sun_km.direction);

            let rayleigh_in_scattering = scattering.rayleigh * rayleigh_phase_value;
            let mie_in_scattering = scattering.mie * mie_phase_value;
            let total_in_scattering = (rayleigh_in_scattering + mie_in_scattering) * sun_transmittance;

            // Integrated scattering within path segment.
            let scattering_integral = (total_in_scattering - total_in_scattering * sample_transmittance) / scattering.total_extinction;

            second_order_scattering += scattering_integral * transmittance;
            transmittance *= sample_transmittance;
        }


        // If we hit the ground, add the ground albedo to the luminance.
        if (ground_distance_km > 0.0 && dot(sample_ray_planet_km.origin, ray_to_sun_km.direction) > 0.0) { // TODO: isn't that just pos.y * sun_dir.y > 0.0?
            let sun_transmittance_at_ground = sample_transmittance_lut(lut_transmittance, 0.0, ray_to_sun_km.direction);
            second_order_scattering += transmittance * atmosphere_params.ground_albedo * sun_transmittance_at_ground;
        }
    }

    multiple_scattering_factor /= f32(DirectionSampleCount);
    second_order_scattering /= f32(DirectionSampleCount);

    // (Equations (10) and (9) from the paper)
    let multiple_scattering = second_order_scattering * (1.0 - multiple_scattering_factor);

    return vec4f(multiple_scattering, 1.0);
}
