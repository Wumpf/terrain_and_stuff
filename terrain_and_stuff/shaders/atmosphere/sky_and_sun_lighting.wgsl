struct SkyAndSunLightingParams {
    // Sun illuminance after scattering through the atmosphere.
    // Sun direction is stored in the global frame uniform buffer for convenient access.
    sun_illuminance: vec3f,

    // Sky luminance in all directions as a order 2 spherical harmonic.
    sky_luminance_sh_coefficients: array<vec3f, 9>,
}
