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

#import "atmosphere/params.wgsl"::{atmosphere_params}
#import "atmosphere/scattering.wgsl"::{scattering_values_for, mie_phase, rayleigh_phase, sample_transmittance_lut}

const NumScatteringSteps: f32 = 64.0;

struct ScatteringResult {
    scattering : vec3f,
    transmittance : vec3f,
}

fn raymarch_scattering(transmittance_lut: texture_2d<f32>, direction: vec3f, planet_relative_position_km: vec3f, dir_to_sun: vec3f, geometry_distance_on_camera_ray: f32) -> ScatteringResult {
    // Figure out where the ray hits either the planet or the atmosphere end.
    // From that we can compute the maximum marching distance in our "regular flat-lander" coordinate system.
    let ray_to_sun_km = Ray(planet_relative_position_km, dir_to_sun);
    let atmosphere_distance_km = ray_sphere_intersect(ray_to_sun_km, atmosphere_params.atmosphere_radius_km);
    let ground_distance_km = ray_sphere_intersect(ray_to_sun_km, atmosphere_params.ground_radius_km);
    let atmosphere_or_ground_distance_km = select(ground_distance_km, atmosphere_distance_km, ground_distance_km < 0.0);
    let max_marching_distance_km = min(atmosphere_or_ground_distance_km, geometry_distance_on_camera_ray * 0.001);


    let cos_theta = dot(direction, dir_to_sun);

    let mie_phase = mie_phase(cos_theta);
    let rayleigh_phase = rayleigh_phase(cos_theta);

    var luminance = vec3f(0.0);
    var transmittance = vec3f(1.0);
    var t = 0.0;

    const sample_segment_t: f32 = 0.3;

    for (var i = 0.0; i < NumScatteringSteps; i += 1.0) {
        let t_new = ((i + sample_segment_t) / NumScatteringSteps) * max_marching_distance_km;
        let dt = t_new - t;
        t = t_new;

        let new_planet_relative_position_km = planet_relative_position_km + t * direction;
        let altitude_km = clamp(length(new_planet_relative_position_km),
                                atmosphere_params.ground_radius_km,
                                atmosphere_params.atmosphere_radius_km) - atmosphere_params.ground_radius_km;

        let scattering = scattering_values_for(altitude_km);
        let sample_transmittance = exp(-dt * scattering.total_extinction);

        let sun_transmittance = sample_transmittance_lut(transmittance_lut, altitude_km, dir_to_sun);
        // TODO: implement multiple scattering LUT.
        let multiscattered_luminance = vec3f(0.0);

        // TODO: earth shadow at night?
        // https://github.com/sebh/UnrealEngineSkyAtmosphere/blob/183ead5bdacc701b3b626347a680a2f3cd3d4fbd/Resources/RenderSkyRayMarching.hlsl#L181
        let planet_shadow = 1.0;
        // TODO: large shadow casters (like mountains or clouds)
        let shadow = 1.0;

        // Compute amount of light coming in via scattering.
        // Note that multi-scattering is unaffected by shadows (approximating it coming from distant sources).
        let phase_times_scattering = scattering.mie * mie_phase + scattering.rayleigh * rayleigh_phase;
        let inscattering = planet_shadow * shadow * sun_transmittance * phase_times_scattering +
                                multiscattered_luminance * (scattering.rayleigh + scattering.mie);
        let inscattering_luminance = atmosphere_params.sun_illuminance * inscattering;

        // Integrated scattering within path segment.
        let scattering_integral = (inscattering_luminance - inscattering_luminance * sample_transmittance) / scattering.total_extinction;

        luminance += scattering_integral * transmittance;
        transmittance *= sample_transmittance;
    }

    var result: ScatteringResult;
    result.transmittance = transmittance;
    result.scattering = luminance;
    return result;
}
