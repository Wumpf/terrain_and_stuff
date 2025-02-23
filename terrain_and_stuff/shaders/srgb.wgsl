// sRGB EOTF.
// Converts a color from 0-1 sRGB to 0-1 linear
fn srgb_eotf(srgb: vec3f) -> vec3f {
    let cutoff = ceil(srgb - 0.04045);
    let under = srgb / 12.92;
    let over = pow((srgb + 0.055) / 1.055,  vec3f(2.4));
    return mix(under, over, cutoff);
}

// sRGB EOTF.
// Converts a color from 0-1 sRGB to 0-1 linear, leaves alpha untouched
fn srgba_eotf(srgb_a: vec4f) -> vec4f {
    return vec4f(srgb_eotf(srgb_a.rgb), srgb_a.a);
}

// sRGB OETF.
// Converts a color from 0-1 linear to 0-1 sRGB
fn srgb_oetf(color_linear: vec3f) -> vec3f {
    let selector = ceil(color_linear - 0.0031308);
    let under = 12.92 * color_linear;
    let over = 1.055 * pow(color_linear, vec3f(0.41666)) - 0.055;
    return mix(under, over, selector);
}

// sRGB OETF.
// Converts a color from 0-1 sRGB to 0-1 linear, leaves alpha untouched
fn srgba_oetf(srgb_a: vec4f) -> vec4f {
    return vec4f(srgb_oetf(srgb_a.rgb), srgb_a.a);
}
