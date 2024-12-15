// TODO: want to put those in a uniform buffer?

// Atmosphere values for earth.
const ground_radius_km: f32 = 6360.0;
const atmosphere_radius_km: f32 = 6460.0;

const rayleigh_scale_height: f32 = 8.0;
const rayleigh_scattering_per_km_density: vec3f = vec3f(0.005802, 0.013558, 0.033100);

const mie_scale_height: f32 = 1.2;
const mie_scattering_per_km_density: f32 = 0.003996;
const mie_absorption_per_km_density: f32 = 0.004440;

const ozone_absorption_per_km_density: vec3f = vec3f(0.000650, 0.001881, 0.000085);
