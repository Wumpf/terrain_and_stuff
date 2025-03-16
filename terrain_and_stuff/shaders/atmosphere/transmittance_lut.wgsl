// Transmittance LUT.
//
// Each pixel coordinate corresponds to a sun zenith angle (x axis) and height (y axis).
// The value is the transmittance from that point to sun, through the atmosphere using single scattering only

#import "constants.wgsl"::{ERROR_RGBA, TAU}
#import "intersections.wgsl"::{ray_sphere_intersect, Ray}

#import "atmosphere/scattering.wgsl"::{scattering_values_for, ScatteringValues}
#import "atmosphere/params.wgsl"::{atmosphere_params}

const SunTransmittanceSteps: f32 = 40.0;

// This is not the parameterization described in http://www.klayge.org/material/4_0/Atmospheric/Precomputed%20Atmospheric%20Scattering.pdf (section 4)
// (and also implemented by https://github.com/JolifantoBambla/webgpu-sky-atmosphere/blob/main/src/shaders/render_transmittance_lut.wgsl)
// Instead just a custom one I whipped up which I found easier to understand and work with.
// Simple parameterization: just map y to height from ground and x to cos(zenith angle).
fn ray_to_sun_texcoord(texcoord: vec2f) -> Ray {
    let sun_cos_theta = pow(2.0 * texcoord.x - 1.0, 5.0); // pow(x, 5.0) for more precision in the middle.
    let sun_theta = acos(sun_cos_theta);
    let height_km = mix(atmosphere_params.ground_radius_km, atmosphere_params.atmosphere_radius_km,
                        texcoord.y * texcoord.y); // square for more precision low altitudes
    let planet_relative_pos = vec3f(0.0, height_km, 0.0);
    let dir_to_sun = normalize(vec3f(0.0, sun_cos_theta, -sin(sun_theta)));

    return Ray(planet_relative_pos, dir_to_sun);
}

@fragment
fn fs_main(@location(0) texcoord: vec2f) -> @location(0) vec4<f32> {
    let ray_to_sun = ray_to_sun_texcoord(texcoord);

    let atmosphere_distance_km = ray_sphere_intersect(ray_to_sun, atmosphere_params.atmosphere_radius_km);
    if atmosphere_distance_km < 0.0 {
        // This should never happen, we're always inside the sphere!
        return ERROR_RGBA;
    }

    var t = 0.0;
    var transmittance = vec3f(0.0);
    const sample_segment_t: f32 = 0.3;
    for (var i = 0.0; i < SunTransmittanceSteps; i += 1.0) {
        let t_new = ((i + sample_segment_t) / SunTransmittanceSteps) * atmosphere_distance_km;
        let dt = t_new - t;
        t = t_new;

        let scattering = scattering_values_for(length(ray_to_sun.origin + t * ray_to_sun.direction) - atmosphere_params.ground_radius_km);
        transmittance += dt * scattering.total_extinction;
    }
    transmittance = exp(-transmittance);

    return vec4f(transmittance, 1.0);
}
