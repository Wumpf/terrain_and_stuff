enable dual_source_blending;

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
#import "camera.wgsl"::{view_space_position_from_depth_buffer, camera_ray_from_screenuv}
#import "global_bindings.wgsl"::{trilinear_sampler_clamp, frame}
#import "intersections.wgsl"::{ray_sphere_intersect, Ray}

#import "atmosphere/scattering.wgsl"::{scattering_values_for, mie_phase, rayleigh_phase}
#import "atmosphere/constants.wgsl"::{
    ground_radius_km,
    atmosphere_radius_km,
    sun_diameteter_rad,
    sun_unscattered_luminance,
}


@group(1) @binding(0)
var transmittance_lut: texture_2d<f32>;

@group(1) @binding(1)
var screen_depth: texture_2d<f32>;

const NumScatteringSteps: f32 = 64.0;

struct ScatteringResult {
    @location(0) @blend_src(0) scattering : vec4f,
    @location(0) @blend_src(1) transmittance : vec4f,
}

fn sample_transmittance_lut(altitude_km: f32, dir_to_sun: vec3f) -> vec3f {
    // See `transmittance_lut.wgsl#ray_to_sun_texcoord` for what it is we're sampling here!
    // u coordinate is mapped to the cos(zenith angle)
    // v coordinate is mapped to the altitude from ground top atmosphere top.
    let sun_cos_zenith_angle = dir_to_sun.y; //dot(dir_to_sun, vec3f(0.0, 1.0, 0.0));
    let relative_altitude = sqrt(altitude_km / (atmosphere_radius_km - ground_radius_km));
    let texcoord = vec2f(pow(sun_cos_zenith_angle, 1.0/5.0) * 0.5 + 0.5, relative_altitude);

    return textureSampleLevel(transmittance_lut, trilinear_sampler_clamp, texcoord, 0.0).rgb;
}

fn raymarch_scattering(camera_ray: Ray, planet_relative_position_km: vec3f, dir_to_sun: vec3f, max_marching_distance_km: f32) -> ScatteringResult {
    let cos_theta = dot(camera_ray.direction, dir_to_sun);

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

        let new_planet_relative_position_km = planet_relative_position_km + t * camera_ray.direction;
        let altitude_km = clamp(length(new_planet_relative_position_km), ground_radius_km, atmosphere_radius_km) - ground_radius_km;

        let scattering = scattering_values_for(altitude_km);
        let sample_transmittance = exp(-dt * scattering.total_extinction);

        let sun_transmittance = sample_transmittance_lut(altitude_km, dir_to_sun);
        // TODO: implement multiple scattering LUT.
        let multiscattered_luminance = vec3f(0.0);

        // TODO: earth shadow at night?
        // https://github.com/sebh/UnrealEngineSkyAtmosphere/blob/183ead5bdacc701b3b626347a680a2f3cd3d4fbd/Resources/RenderSkyRayMarching.hlsl#L181
        let earth_shadow = 1.0;
        // TODO: large shadow casters (like mountains or clouds)
        let shadow = 1.0;

        // Compute amount of light coming in via scattering.
        // Note that multi-scattering is unaffected by shadows (approximating it coming from distant sources).
        let phase_times_scattering = scattering.mie * mie_phase + scattering.rayleigh * rayleigh_phase;
        let inscattering = earth_shadow * shadow * sun_transmittance * phase_times_scattering +
                                multiscattered_luminance * (scattering.rayleigh + scattering.mie);

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

    var result: ScatteringResult;
    result.transmittance = vec4f(transmittance, 1.0);
    result.scattering = vec4f(luminance, 1.0);
    return result;
}

@fragment
fn fs_main(@location(0) texcoord: vec2f, @builtin(position) position: vec4f) -> ScatteringResult {
    let camera_ray = camera_ray_from_screenuv(texcoord);

    // Determine the length of the camera ray when we hit geometry - this length is infinity wherever we hit the sky.
    let view_space_position = view_space_position_from_depth_buffer(textureLoad(screen_depth, vec2i(position.xy), 0).r, texcoord);
    let geometry_distance_on_camera_ray = length(view_space_position);

    let dir_to_sun = normalize(vec3f(0.0, 10.0, 30.0)); // TODO:

    // For our camera we generally assume a flat planet.
    // But as we march through the atmosphere, we have to take into account that the atmosphere is curved.
    let planet_relative_position_km = vec3(0.0, max(0.0, camera_ray.origin.y * 0.001) + ground_radius_km, 0.0);

    // Figure out where the ray hits either the planet or the atmosphere end.
    // From that we can compute the maximum marching distance in our "regular flat-lander" coordinate system.
    let ray_to_sun_km = Ray(planet_relative_position_km, dir_to_sun);
    let atmosphere_distance_km = ray_sphere_intersect(ray_to_sun_km, atmosphere_radius_km);
    let ground_distance_km = ray_sphere_intersect(ray_to_sun_km, ground_radius_km);
    let atmosphere_or_ground_distance_km = select(ground_distance_km, atmosphere_distance_km, ground_distance_km < 0.0);
    let max_marching_distance_km = min(atmosphere_or_ground_distance_km, geometry_distance_on_camera_ray * 0.001);

    let result = raymarch_scattering(camera_ray, planet_relative_position_km, dir_to_sun, max_marching_distance_km);

    // WORKAROUND FOR CHROME:
    // Check this last, so everything above is uniform control flow.
    // (https://www.w3.org/TR/WGSL/#fwidth-builtin is supposed to return an indeterminate value in this case but accept the shader)
    if atmosphere_distance_km < 0.0 {
        // This shader isn't equipped for views outside of the atmosphere.
        return ScatteringResult(ERROR_RGBA, ERROR_RGBA);
    }

    return result;

    // Debug stuff:
    //return ScatteringResult(vec4f(fract(max_marching_distance_km * 0.1)), vec4f(0.0));

    //let world_space_position = (view_space_position * frame.view_from_world).xyz + frame.camera_position;
    //return ScatteringResult(vec4f(fract(abs(world_space_position) * 0.0001), 1.0), vec4f(0.0));

    //let trasmittance_lut = textureSample(transmittance_lut, trilinear_sampler_clamp, texcoord);
    //return ScatteringResult(trasmittance_lut, vec4f(0.0));
}
