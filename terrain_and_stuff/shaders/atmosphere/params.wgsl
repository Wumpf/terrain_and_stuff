// See atmosphere.rs#AtmosphereDebugDrawMode
const AtmosphereDebugDrawMode_None: u32 = 0;
const AtmosphereDebugDrawMode_Sh: u32 = 1;
const AtmosphereDebugDrawMode_NoScatteringOverlay: u32 = 2;

struct AtmosphereParams {
    debug_draw_mode: u32,

    ground_radius_km: f32,
    atmosphere_radius_km: f32,

    rayleigh_scale_height: f32,
    // -- row boundary --
    rayleigh_scattering_per_km_density: vec3f,


    mie_scale_height: f32,
    // -- row boundary --
    mie_scattering_per_km_density: f32,
    mie_absorption_per_km_density: f32,

    sun_disk_diameteter_rad: f32,
    sun_disk_illuminance_factor: f32,
    // -- row boundary --

    ozone_absorption_per_km_density: vec3f,
    // -- row boundary --

    sun_illuminance: vec3f,
}

@group(1) @binding(0) var<uniform> atmosphere_params: AtmosphereParams;