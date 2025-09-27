#import "global_bindings.wgsl"::{trilinear_sampler_clamp}
#import "constants.wgsl"::{TAU, PI}
#import "atmosphere/params.wgsl"::{atmosphere_params}

struct ScatteringValues {
    rayleigh: vec3f,
    mie: f32,
    total_extinction_per_km: vec3f,
}

/// Computes rayleigh, mie and extinction values for an altitude in the atmosphere.
fn scattering_values_for(altitude_km: f32) -> ScatteringValues {
    // We take three different light interaction into account:
    // - particles smaller than a wavelength: Rayleigh scattering
    // - larger particles: Mie scattering & absorption
    // - absorption effect of ozone

    let rayleigh_density = exp(-altitude_km / atmosphere_params.rayleigh_scale_height);
    let mie_density = exp(-altitude_km / atmosphere_params.mie_scale_height);
    let ozone_density = max(0.0, 1.0 - abs(altitude_km - 25.0) / 15.0);

    var scattering: ScatteringValues;
    scattering.rayleigh = atmosphere_params.rayleigh_scattering_per_km_density * rayleigh_density;
    scattering.mie = atmosphere_params.mie_scattering_per_km_density * mie_density;
    // Ozone has no scattering contribution.
    let total_scattering = scattering.rayleigh + scattering.mie;

    let mie_absorption = atmosphere_params.mie_absorption_per_km_density * mie_density;
    let ozone_absorption = atmosphere_params.ozone_absorption_per_km_density * ozone_density;
    // Rayleigh has no absorption contribution.
    let total_absorption = mie_absorption + ozone_absorption;

    // The amount of light we "loose" on a straight path is called extinction.
    // -> part of the light is absorbed, part of it is scattered!
    scattering.total_extinction_per_km = total_scattering + total_absorption;

    return scattering;
}

fn mie_phase(cos_theta: f32) -> f32 {
    const g = 0.8;
    const scale = 3.0/ (4.0 * TAU);

    let num = (1.0 - g*g) * (1.0 + cos_theta * cos_theta);
    let denom = (2.0 + g*g) * pow((1.0 + g*g - 2.0 * g * cos_theta), 1.5);

    return scale * num / denom;
}

fn rayleigh_phase(cos_theta: f32) -> f32 {
    const k = 3.0 / (8.0 * TAU);
    return k * (1.0 + cos_theta * cos_theta);
}

fn sample_transmittance_lut(transmittance_lut: texture_2d<f32>, altitude_km: f32, sun_cos_zenith_angle: f32) -> vec3f {
    // See `transmittance_lut.wgsl#ray_to_sun_texcoord` for what it is we're sampling here!
    // u coordinate is mapped to the cos(zenith angle)
    // v coordinate is mapped to the altitude from ground top atmosphere top.
    let relative_altitude = sqrt(altitude_km / (atmosphere_params.atmosphere_radius_km - atmosphere_params.ground_radius_km));

    //let packed_zenith_angle = sun_cos_zenith_angle * 0.5 + 0.5;
    let packed_zenith_angle = (0.25 * PI) * sin(sun_cos_zenith_angle) + 0.5;

    let texcoord = vec2f(packed_zenith_angle, relative_altitude);

    return textureSampleLevel(transmittance_lut, trilinear_sampler_clamp, texcoord, 0.0).rgb;
}
