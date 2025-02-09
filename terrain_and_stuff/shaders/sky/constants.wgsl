#import "constants.wgsl"::{DEG_TO_RAD}

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

// Sun's angle is 0.5 degrees according to this.
// https://www.nasa.gov/wp-content/uploads/2015/01/YOSS_Act_9.pdf
const sun_diameteter_rad = 0.5 * DEG_TO_RAD;

// Roughly the intensity Sun without any scattering
// https://en.wikipedia.org/wiki/Luminance
//const sun_unscattered_luminance: vec3f = vec3f(1.6, 1.6, 1.6) * 1000000000.0;
// Okay that's just too much to work with 🤷
const sun_unscattered_luminance: vec3f = vec3f(1.6, 1.6, 1.6) * 1000.0;
