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

#import "intersections.wgsl"::{Ray, ray_sphere_intersect}
#import "global_bindings.wgsl"::{trilinear_sampler_clamp}

#import "atmosphere/params.wgsl"::{atmosphere_params}
#import "atmosphere/scattering.wgsl"::{scattering_values_for, mie_phase, rayleigh_phase, sample_transmittance_lut}

const NumScatteringSteps: f32 = 64.0;

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

fn raymarch_scattering(transmittance_lut: texture_2d<f32>,
                        multiple_scattering_lut: texture_2d<f32>,
                        direction: vec3f,
                        planet_relative_position_km: vec3f,
                        dir_to_sun: vec3f,
                        geometry_distance_on_camera_ray: f32
                    ) -> ScatteringResult {

    let ray_to_sun_km = Ray(planet_relative_position_km, dir_to_sun);
    let max_marching_distance_km = max_marching_distance_km(ray_to_sun_km, geometry_distance_on_camera_ray);

    let cos_theta = dot(direction, dir_to_sun);

    let mie_phase = mie_phase(cos_theta);
    let rayleigh_phase = rayleigh_phase(cos_theta);

    var luminance = vec3f(0.0);
    var transmittance = vec3f(1.0);

    // TODO: Using a fixed sample count right now, but maybe we should use a dynamic one depending on the distance we're marching?
    // TODO: randomize sample offsets would help a lot here.
    let dt = max_marching_distance_km / NumScatteringSteps;
    var t = 0.0;
    const sample_segment_t: f32 = 0.3;

    for (var i = 0.0; i < NumScatteringSteps; i += 1.0) {
        let t_new = (i + sample_segment_t) * dt;
        let dt_exact = t_new - t;
        t = t_new;

        let new_planet_relative_position_km = planet_relative_position_km + t * direction;
        let sample_height = length(new_planet_relative_position_km);
        let altitude_km = clamp(sample_height, atmosphere_params.ground_radius_km,
                                atmosphere_params.atmosphere_radius_km) - atmosphere_params.ground_radius_km;
        let zenith = new_planet_relative_position_km / sample_height;
        let sun_cos_zenith_angle = dot(zenith, dir_to_sun);

        let scattering = scattering_values_for(altitude_km);
        let sample_transmittance = exp(-dt_exact * scattering.total_extinction_per_km);

        let sun_transmittance = sample_transmittance_lut(transmittance_lut, altitude_km, sun_cos_zenith_angle);


        // TODO: earth shadow at night?
        // https://github.com/sebh/UnrealEngineSkyAtmosphere/blob/183ead5bdacc701b3b626347a680a2f3cd3d4fbd/Resources/RenderSkyRayMarching.hlsl#L181
        // let t_earth = ray_sphere_intersect(Ray(new_planet_relative_position_km, -dir_to_sun), atmosphere_params.ground_radius_km);
        // let planet_shadow = f32(t_earth >= 0.0);
        let planet_shadow = 1.0;

        // TODO: large shadow casters (like mountains or clouds)
        let shadow = 1.0;

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
