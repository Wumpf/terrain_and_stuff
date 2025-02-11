#import "constants.wgsl"::{TAU}
#import "atmosphere/constants.wgsl"::{
    ground_radius_km,
    rayleigh_scale_height,
    rayleigh_scattering_per_km_density,
    mie_scale_height,
    mie_scattering_per_km_density,
    mie_absorption_per_km_density,
    ozone_absorption_per_km_density
}

struct ScatteringValues {
    rayleigh: vec3f,
    mie: f32,
    total_extinction: vec3f,
}

/// Computes rayleigh, mie and extinction values for an altitude in the atmosphere.
fn scattering_values_for(altitude_km: f32) -> ScatteringValues {
    let rayleigh_density = exp(-altitude_km / rayleigh_scale_height);
    let mie_density = exp(-altitude_km / mie_scale_height);

    var scattering: ScatteringValues;
    scattering.rayleigh = rayleigh_scattering_per_km_density * rayleigh_density;
    scattering.mie = mie_scattering_per_km_density * mie_density;

    let mie_absorption = mie_absorption_per_km_density * mie_density;
    let ozone_absorption = ozone_absorption_per_km_density * max(0.0, 1.0 - abs(altitude_km - 25.0) / 15.0);
    scattering.total_extinction = scattering.rayleigh + scattering.mie + mie_absorption + ozone_absorption;

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
