// Transmittance LUT.
//
// Each pixel coordinate corresponds to a sun zenith angle (x axis) and height (y axis).
// The value is the transmittance from that point to sun, through the atmosphere using single scattering only

#import "constants.wgsl"::{ERROR_RGBA}
#import "intersections.wgsl"::{ray_sphere_intersect, Ray}

#import "sky/scattering.wgsl"::{scattering_values_for, ScatteringValues}
#import "sky/constants.wgsl"::{
    ground_radius_km,
    atmosphere_radius_km,
}

const SunTransmittanceSteps: f32 = 40.0;


@fragment
fn fs_main(@location(0) texcoord: vec2f) -> @location(0) vec4<f32> {
    // TODO: This is not the parameterization described in http://www.klayge.org/material/4_0/Atmospheric/Precomputed%20Atmospheric%20Scattering.pdf (section 4)
    // (and also implemented by https://github.com/JolifantoBambla/webgpu-sky-atmosphere/blob/main/src/shaders/render_transmittance_lut.wgsl)
    // But instead a much simpler one that just maps y to height from ground and x to cos(zenith angle).
    // Tbh I don't fully understand the more complicated one below yet, I might as well whip up something myself and see how close it ends up being to that :)
    // (really should because this is terribly wasteful!)
    let sun_cos_theta = 2.0 * texcoord.x - 1.0;
    let sun_theta = acos(sun_cos_theta);
    let height_km = mix(ground_radius_km, atmosphere_radius_km, texcoord.y);
    let planet_relative_pos = vec3f(0.0, height_km, 0.0);
    let dir_to_sun = normalize(vec3f(0.0, sun_cos_theta, -sin(sun_theta)));

    // let ground_radius_sq = ground_radius_km * ground_radius_km;
    // let h_sq = atmosphere_radius_km * atmosphere_radius_km - ground_radius_sq;
    // let h = sqrt(h_sq);
    // let rho = sqrt(h_sq) * texcoord.y;
    // let rho_sq = rho * rho;
    // let height = sqrt(rho_sq + ground_radius_sq);

    // let d_min = atmosphere_radius_km - height;
    // let d_max = rho + h;
    // let d = d_min + texcoord.x * (d_max - d_min);
    // let cos_view_zenith = clamp((h_sq - rho_sq - d * d) / (2.0 * height * d), -1.0, 1.0);

    // let planet_relative_pos = vec3f(0.0, height, 0.0);
    // let dir_to_sun = normalize(vec3f(0.0, cos_view_zenith, sqrt(1.0 - cos_view_zenith * cos_view_zenith)));



    let atmosphere_distance_km = ray_sphere_intersect(Ray(planet_relative_pos, dir_to_sun), atmosphere_radius_km);
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

        let scattering = scattering_values_for(length(planet_relative_pos + t * dir_to_sun) - ground_radius_km);
        transmittance += dt * scattering.total_extinction;
    }
    transmittance = exp(-transmittance);

    return vec4f(transmittance, 1.0);
}
