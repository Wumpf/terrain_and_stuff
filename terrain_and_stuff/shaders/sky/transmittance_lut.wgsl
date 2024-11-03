// Transmittance LUT.
//
// Each pixel coordinate corresponds to a sun zenith angle (x axis) and height (y axis).
// The value is the transmittance from that point to sun, through the atmosphere using single scattering only

#import "intersections.wgsl"::{ray_sphere_intersect, Ray}

const SunTransmittanceSteps: f32 = 40.0;

// Atmosphere values for earth.
const ground_radius_km: f32 = 6360.0;
const atmosphere_radius_km: f32 = 6460.0;

const rayleigh_scale_height: f32 = 8.0;
const rayleigh_scattering_per_km_density: vec3f = vec3f(0.005802, 0.013558, 0.033100);

const mie_scale_height: f32 = 1.2;
const mie_scattering_per_km_density: f32 = 0.003996;
const mie_absorption_per_km_density: f32 = 0.004440; 

const ozone_absorption_per_km_density: vec3f = vec3f(0.000650, 0.001881, 0.000085);

@fragment
fn fs_main(@location(0) texcoord: vec2f) -> @location(0) vec4<f32> {
    // TODO: This is not the parameterization described in http://www.klayge.org/material/4_0/Atmospheric/Precomputed%20Atmospheric%20Scattering.pdf (section 4)
    // (and also implemented by https://github.com/JolifantoBambla/webgpu-sky-atmosphere/blob/main/src/shaders/render_transmittance_lut.wgsl)
    let sun_cos_theta = 2.0 * texcoord.x - 1.0;
    let sun_theta = acos(sun_cos_theta);
    let height = mix(ground_radius_km, atmosphere_radius_km, texcoord.y);

    let pos = vec3f(0.0, height, 0.0); 
    let dir_to_sun = normalize(vec3f(0.0, sun_cos_theta, -sin(sun_theta)));
    
    let atmosphere_distance_km = ray_sphere_intersect(Ray(pos, dir_to_sun), atmosphere_radius_km);
    if atmosphere_distance_km < 0.0 {
        // This should never happen, we're always inside the sphere!
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }

    var t = 0.0;
    var transmittance = vec3f(0.0);
    const sample_segment_t: f32 = 0.3;
    for (var i = 0.0; i < SunTransmittanceSteps; i += 1.0) {
        let t_new = ((i + sample_segment_t) / SunTransmittanceSteps) * atmosphere_distance_km;
        let dt = t_new - t;
        t = t_new;
     
        let scattering = scattering_values_for(pos + t * dir_to_sun);
        transmittance += dt * scattering.total_extinction;
    }
    transmittance = exp(-transmittance);

    return vec4<f32>(abs(sun_theta), 0.0, 0.0, 1.0); 
}

struct ScatteringValues {
    rayleigh: vec3f,
    mie: f32,
    total_extinction: vec3f,
}

/// Computes rayleigh, mie and extinction values for a given position in the atmosphere.
fn scattering_values_for(pos: vec3f) -> ScatteringValues {
    let altitude_km = length(pos) - ground_radius_km;

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
