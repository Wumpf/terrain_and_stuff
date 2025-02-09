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

#import "constants.wgsl"::{ERROR_RGBA}
#import "camera.wgsl"::{camera_ray_from_screenuv}
#import "global_bindings.wgsl"::{trilinear_sampler_clamp, frame}
#import "intersections.wgsl"::{ray_sphere_intersect, Ray}

#import "sky/scattering.wgsl"::{scattering_values_for, mie_phase, rayleigh_phase}
#import "sky/constants.wgsl"::{
    ground_radius_km,
    atmosphere_radius_km,
    sun_diameteter_rad,
    sun_unscattered_luminance,
}


@group(1) @binding(0)
var transmittance_lut: texture_2d<f32>;

const NumScatteringSteps: f32 = 64.0;

fn sample_transmittance_lut(altitude_km: f32, dir_to_sun: vec3f) -> vec3f {
    // See `transmittance_lut.wgsl#ray_to_sun_texcoord` for what it is we're sampling here!
    // u coordinate is mapped to the cos(zenith angle)
    // v coordinate is mapped to the altitude from ground top atmosphere top.
    let sun_cos_zenith_angle = dir_to_sun.y; //dot(dir_to_sun, vec3f(0.0, 1.0, 0.0));
    let relative_altitude = sqrt(altitude_km / (atmosphere_radius_km - ground_radius_km));
    let texcoord = vec2f(pow(sun_cos_zenith_angle, 1.0/5.0) * 0.5 + 0.5, relative_altitude);

    return textureSampleLevel(transmittance_lut, trilinear_sampler_clamp, texcoord, 0.0).rgb;
}

fn raymarch_scattering(camera_ray: Ray, dir_to_sun: vec3f, max_marching_distance_km: f32) -> vec3f {
    let cos_theta = dot(camera_ray.direction, dir_to_sun);

    let mie_phase = mie_phase(cos_theta);
    let rayleigh_phase = rayleigh_phase(cos_theta);

    var luminance = vec3f(0.0);
    var transmittance = vec3f(1.0);
    var t = 0.0;

    const sample_segment_t: f32 = 0.3;

    // For our camera we generally assume a flat planet.
    // But as we march through the atmosphere, we have to take into account that the atmosphere is curved.
    let planet_relative_position_km = vec3(0.0, camera_ray.origin.y * 0.001 + ground_radius_km, 0.0);

    for (var i = 0.0; i < NumScatteringSteps; i += 1.0) {
        let t_new = ((i + sample_segment_t) / NumScatteringSteps) * max_marching_distance_km;
        let dt = t_new - t;
        t = t_new;

        let new_planet_relative_position_km = planet_relative_position_km + t * camera_ray.direction;
        let altitude_km = max(0.0, length(new_planet_relative_position_km) - ground_radius_km);

        let scattering = scattering_values_for(altitude_km);
        let sample_transmittance = exp(-dt * scattering.total_extinction);

        let sun_transmittance = sample_transmittance_lut(altitude_km, dir_to_sun);
        // TODO: implement multiple scattering LUT.
        let psi_multiple_scattering = vec3f(0.0);

        // TODO: earth shadow at night?
        // https://github.com/sebh/UnrealEngineSkyAtmosphere/blob/183ead5bdacc701b3b626347a680a2f3cd3d4fbd/Resources/RenderSkyRayMarching.hlsl#L181

        let rayleigh_inscattering = scattering.rayleigh * (rayleigh_phase * sun_transmittance + psi_multiple_scattering);
        let mie_inscattering = scattering.mie * (mie_phase * sun_transmittance + psi_multiple_scattering);
        let inscattering = rayleigh_inscattering + mie_inscattering;

        // TODO:
        //let inscattering = vec3f(0.0);

        // Integrated scattering within path segment.
        let scattering_integral = (inscattering - inscattering * sample_transmittance) / scattering.total_extinction;

        luminance += scattering_integral * transmittance;
        transmittance *= sample_transmittance;
    }

    // Add sun contribution
    let sun = dot(camera_ray.direction, dir_to_sun) - cos(sun_diameteter_rad);
    // Since the sun is so bright, this isn't giving us enough antialiasing yet.
    //let antialiased_sun = saturate(sun / (fwidth(sun) * 100.0));
    // Fudging this with a looks good enough.
    let antialiased_sun = saturate(sun / (fwidth(sun) * 1000.0));
    luminance += sun_unscattered_luminance * transmittance * antialiased_sun;

    return luminance;
}

@fragment
fn fs_main(@location(0) texcoord: vec2f) -> @location(0) vec4<f32> {
    let camera_ray = camera_ray_from_screenuv(texcoord);

    let dir_to_sun = normalize(vec3f(0.0, 2.0, 30.0)); // TODO:

    // Figure out where the ray hits either the planet or the atmosphere end.
    // From that we can compute the maximum marching distance in our "regular flat-lander" coordinate system.
    let pos_on_planet_km = (vec3f(0.0, ground_radius_km, 0.0) + camera_ray.origin * 0.001);
    let ray_to_sun = Ray(pos_on_planet_km, dir_to_sun);
    let atmosphere_distance_km = ray_sphere_intersect(ray_to_sun, atmosphere_radius_km);
    if atmosphere_distance_km < 0.0 {
        // This shader isn't equipped for views outside of the atmosphere.
        return ERROR_RGBA;
    }
    let ground_distance_km = ray_sphere_intersect(ray_to_sun, ground_radius_km);
    let max_marching_distance_km = select(ground_distance_km, atmosphere_distance_km, ground_distance_km < 0.0);


    let luminance = raymarch_scattering(camera_ray, dir_to_sun, max_marching_distance_km);
    return vec4f(luminance, 1.0);

    // DEBUG:
    //return textureSampleLevel(transmittance_lut, trilinear_sampler_clamp, texcoord, 0.0);

    //return vec4f(camera_ray.direction, 1.0);
}
