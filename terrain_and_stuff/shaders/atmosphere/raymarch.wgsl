// Raymarch the sky color using transmittance & multiple scattering luts.
//
// This is what gives us the final sky color and light scattering overlay.
//
// The original technique describes also how to put this into a lat-long lookup texture.
// Doing so is a lot more efficient and screen resolution decoupled!
// However, this makes any interaction with occluders an approximation.
// The "Aerial Perspective LUT" (a volume texture for storing luminance & transmittance) use for this
// in the paper is a very good approximation, but naturally doesn't quite reach the quality of full per-pixel raymarching.

// SebH uses photometric units rather than radiometric units.
// Output units are more akin to radiometric units though:
// (See https://www.reedbeta.com/blog/radiometry-versus-photometry/)
// As explained in https://seblagarde.wordpress.com/wp-content/uploads/2015/07/course_notes_moving_frostbite_to_pbr_v32.pdf
// it has some advantages to keep all units radiometric for non-spectral rendering and a fixed conversion factor can be assumed.
// To my (rather poor) understanding, there's some handwaviness around what the R/G/B values
// for our photometric units actually mean - see also Nathan's comment here on recommending to cheat
// by using fixed wavelengths rather spectra https://computergraphics.stackexchange.com/a/1994

import package::global_bindings::{frame_uniforms};
import package::intersections::{Ray, ray_sphere_intersect};
import package::global_bindings::{trilinear_sampler_clamp};

import package::atmosphere::params::{atmosphere_params};
import package::atmosphere::scattering::{scattering_values_for, mie_phase, rayleigh_phase, sample_transmittance_lut};

const NumScatteringSteps: f32 = 32.0;

struct ScatteringResult {
    scattering : vec3f,
    transmittance : vec3f,
}

fn sample_multiple_scattering_lut(multiple_scattering_lut: texture_2d<f32>,
                                  altitude_km: f32,
                                  sun_cos_zenith_angle: f32) -> vec3f {
    let relative_altitude = altitude_km / (atmosphere_params.atmosphere_radius_km - atmosphere_params.ground_radius_km);
    let texcoord = vec2f(sun_cos_zenith_angle * 0.5 + 0.5, relative_altitude);

    return textureSampleLevel(multiple_scattering_lut, trilinear_sampler_clamp, texcoord, 0.0).xyz;
}

fn max_marching_distance_km(ray_to_sun_km: Ray, geometry_distance_on_camera_ray: f32) -> f32 {
    // Figure out where the ray hits either the planet or the edge of the atmosphere.
    // From that we can compute the maximum marching distance in our "regular flat-lander" coordinate system.

    var atmosphere_or_ground_distance_km = ray_sphere_intersect(ray_to_sun_km, atmosphere_params.atmosphere_radius_km);

    let inside_planet = dot(ray_to_sun_km.origin, ray_to_sun_km.direction) <= atmosphere_params.ground_radius_km * atmosphere_params.ground_radius_km;
    if !inside_planet {
        let ground_distance_km = ray_sphere_intersect(ray_to_sun_km, atmosphere_params.ground_radius_km);
        if ground_distance_km > 0.0 {
            atmosphere_or_ground_distance_km = ground_distance_km;
        }
    }

    return min(atmosphere_or_ground_distance_km, geometry_distance_on_camera_ray * 0.001);
}

struct Segment {
    t: f32,
    dt: f32,
}

fn compute_segment(i: f32, sample_segment_t: f32, inv_num_scattering_steps: f32, max_marching_distance_km: f32) -> Segment {
    var segment: Segment;
    // Quadratic distance sampling for much higher precision up close.
    var t0 = i * inv_num_scattering_steps;
    var t1 = t0 + inv_num_scattering_steps;
    t1 = (t1 * t1) * max_marching_distance_km;
    t0 = (t0 * t0) * max_marching_distance_km;

    segment.dt = t1 - t0;
    // We don't use `sample_segment_t` as a fixed offset, but rather as a percentage of the current segment
    // (with segments becoming larger quadratically larger)
    segment.t = t0 + segment.dt * sample_segment_t;

    return segment;
}

// `sample_segment_t` determines where along the segment we sample transmittance and scattering.
// It's expected to be a random value in range 0-1.
fn raymarch_scattering(sample_segment_t: f32,
                        transmittance_lut: texture_2d<f32>,
                        multiple_scattering_lut: texture_2d<f32>,
                        direction: vec3f,
                        planet_relative_position_km: vec3f,
                        geometry_distance_on_camera_ray: f32,
                        @if(SAMPLE_SHADOW) shadow_map: texture_depth_2d,
                        @if(SAMPLE_SHADOW) shadow_sampler: sampler_comparison,
                    ) -> ScatteringResult {

    let ray_to_sun_km = Ray(planet_relative_position_km, frame_uniforms.dir_to_sun);
    let max_marching_distance_km = max_marching_distance_km(ray_to_sun_km, geometry_distance_on_camera_ray);

    let cos_theta = dot(direction, frame_uniforms.dir_to_sun);

    let mie_phase = mie_phase(cos_theta);
    let rayleigh_phase = rayleigh_phase(cos_theta);

    var luminance = vec3f(0.0);
    var transmittance = vec3f(1.0);

    // TODO: Using a fixed sample count right now, but maybe we should use a dynamic one depending on the distance we're marching?
    let inv_num_scattering_steps = 1.0 / NumScatteringSteps;
    for (var i = 0.0; i < NumScatteringSteps; i += 1.0) {
        let s = compute_segment(i, sample_segment_t, inv_num_scattering_steps, max_marching_distance_km);

        let new_planet_relative_position_km = planet_relative_position_km + s.t * direction;
        let sample_height = length(new_planet_relative_position_km);
        let altitude_km = clamp(sample_height, atmosphere_params.ground_radius_km,
                                atmosphere_params.atmosphere_radius_km) - atmosphere_params.ground_radius_km;
        let zenith = new_planet_relative_position_km / sample_height;
        let sun_cos_zenith_angle = dot(zenith, frame_uniforms.dir_to_sun);

        let scattering = scattering_values_for(altitude_km);
        let sample_transmittance = exp(-s.dt * scattering.total_extinction_per_km);

        let sun_transmittance = sample_transmittance_lut(transmittance_lut, altitude_km, sun_cos_zenith_angle);


        // TODO: earth shadow at night?
        // https://github.com/sebh/UnrealEngineSkyAtmosphere/blob/183ead5bdacc701b3b626347a680a2f3cd3d4fbd/Resources/RenderSkyRayMarching.hlsl#L181
        // let t_earth = ray_sphere_intersect(Ray(new_planet_relative_position_km, -frame_uniforms.dir_to_sun), atmosphere_params.ground_radius_km);
        // let planet_shadow = f32(t_earth >= 0.0);
        let planet_shadow = 1.0;

        // Sample primary shadow map.
        var shadow = 1.0;
        @if(SAMPLE_SHADOW)
        {
            let world_position = (new_planet_relative_position_km - vec3f(0.0, atmosphere_params.ground_radius_km, 0.0)) * 1000.0;
            let shadow_proj = (frame_uniforms.shadow_map_from_world * vec4f(world_position, 1.0));
            if shadow_proj.x <= 1.0 && shadow_proj.x >= -1.0 &&
               shadow_proj.y <= 1.0 && shadow_proj.y >= -1.0 &&
               shadow_proj.z <= 1.0 && shadow_proj.z >= 0.0 {
                shadow = textureSampleCompare(shadow_map, shadow_sampler, shadow_proj.xy * vec2(0.5, -0.5) + vec2f(0.5), shadow_proj.z);
            }
        }

        // Compute amount of light coming in via scattering.
        let phase_times_scattering = scattering.mie * mie_phase + scattering.rayleigh * rayleigh_phase;
        var inscattering = planet_shadow * shadow * sun_transmittance * phase_times_scattering;

        if atmosphere_params.enable_multiple_scattering != 0 {
             // Multi-scattering is unaffected by shadows (approximating it coming from distant sources).
            let multiscattered_luminance = sample_multiple_scattering_lut(multiple_scattering_lut, altitude_km, sun_cos_zenith_angle);
            inscattering += multiscattered_luminance * (scattering.rayleigh + scattering.mie);
        }

        let inscattering_luminance = atmosphere_params.sun_illuminance * inscattering;

        // Integrated scattering within path segment.
        let scattering_integral = (inscattering_luminance - inscattering_luminance * sample_transmittance) / scattering.total_extinction_per_km;

        luminance += scattering_integral * transmittance;
        transmittance *= sample_transmittance;
    }

    var result: ScatteringResult;
    result.transmittance = transmittance;
    result.scattering = luminance;
    return result;
}
