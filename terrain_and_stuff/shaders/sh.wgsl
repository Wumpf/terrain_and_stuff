// Functions & data structures for spherical harmonics.

const SH_FACTOR_BAND0: f32 = 0.282094792; // 1.0 / (2.0 * sqrt(PI))

const SH_FACTOR_BAND1: f32 = 0.488602512;      // sqrt(3.0) / (2.0 * sqrt(PI))
const SH_FACTOR_BAND2_non0: f32 = 1.092548431; // sqrt(15.0) / (2.0 * sqrt(PI))
const SH_FACTOR_BAND2_0: f32 = 0.315391565;    // sqrt(5.0) / (4.0 * sqrt(PI))

const SH_FACTOR_BAND3_0: f32 = 0.373176336;
const SH_FACTOR_BAND3_1: f32 = 0.457045794;
const SH_FACTOR_BAND3_2: f32 = 2.89061141;
const SH_FACTOR_BAND3_3: f32 = 0.590043604;

const SH_FACTOR_COSINE_BAND0: f32 = 0.886226925; // PI / (2.0 * sqrt(PI))

const SH_FACTOR_COSINE_BAND1: f32 = 1.023326708;      // 2.0 * PI * sqrt(3.0) / (6.0 * sqrt(PI))
const SH_FACTOR_COSINE_BAND2_non0: f32 = 0.858085531; // PI * sqrt(15.0) / (8.0 * sqrt(PI))
const SH_FACTOR_COSINE_BAND2_0: f32 = 0.247707956;    // PI * sqrt(5.0) / (16.0 * sqrt(PI))

fn sh_weight_00(dir: vec3f) -> f32 {
    return SH_FACTOR_BAND0;
}

fn sh_weight_1n1(dir: vec3f) -> f32 {
    return -SH_FACTOR_BAND1 * dir.y;
}

fn sh_weight_10(dir: vec3f) -> f32 {
    return SH_FACTOR_BAND1 * dir.z;
}

fn sh_weight_1p1(dir: vec3f) -> f32 {
    return -SH_FACTOR_BAND1 * dir.x;
}

fn sh_weight_2n2(dir: vec3f) -> f32 {
    return SH_FACTOR_BAND2_non0 * dir.y * dir.x;
}

fn sh_weight_2n1(dir: vec3f) -> f32 {
    return -SH_FACTOR_BAND2_non0 * dir.y * dir.z;
}

fn sh_weight_20(dir: vec3f) -> f32 {
    return SH_FACTOR_BAND2_0 * (3.0 * dir.z * dir.z - 1.0);
}

fn sh_weight_2p1(dir: vec3f) -> f32 {
    return -SH_FACTOR_BAND2_non0 * dir.x * dir.z;
}

fn sh_weight_2p2(dir: vec3f) -> f32 {
    return SH_FACTOR_BAND2_non0 * 0.5 * (dir.x * dir.x - dir.y * dir.y);
}


// Spherical harmonics coefficients for bands 0, 1, 2.
// (as vec3 colors)
// TODO: Pack without padding?
//
// Naga oil struggles with exporting the type either way, so can't repeat this struct, use flat array instead without alias.
// Why is this?
//
// struct SHCoefficientsBands0to2 {
//     band0: vec3f,
//     band1: array<vec3f, 3>,
//     band2: array<vec3f, 5>,
// }
// alias SHCoefficientsBands0to2 = array<vec3f, 9>;//1 + 3 + 5>;


/// Evaluate the first three bands of spherical harmonics in a given direction.
fn evaluate_sh2(dir: vec3f, coeffs: array<vec3f, 9>) -> vec3f {
    var result = coeffs[0] * sh_weight_00(dir);

    result += coeffs[1] * sh_weight_1n1(dir);
    result += coeffs[2] * sh_weight_10(dir);
    result += coeffs[3] * sh_weight_1p1(dir);

    result += coeffs[4] * sh_weight_2n2(dir);
    result += coeffs[5] * sh_weight_2n1(dir);
    result += coeffs[6] * sh_weight_20(dir);
    result += coeffs[7] * sh_weight_2p1(dir);
    result += coeffs[8] * sh_weight_2p2(dir);

    return max(vec3f(0.0), result);
}

/// Evaluate the first three bands of spherical harmonics for a cosine lobe in the given direction.
fn evaluate_sh2_cosine(dir: vec3f, coeffs: array<vec3f, 9>) -> vec3f {
    var result = coeffs[0] * SH_FACTOR_COSINE_BAND0;

    result += coeffs[1] * (-SH_FACTOR_COSINE_BAND1 * dir.y);
    result += coeffs[2] * (SH_FACTOR_COSINE_BAND1 * dir.z);
    result += coeffs[3] * (-SH_FACTOR_COSINE_BAND1 * dir.x);

    result += coeffs[4] * (SH_FACTOR_COSINE_BAND2_non0 * dir.y * dir.x);
    result += coeffs[5] * (-SH_FACTOR_COSINE_BAND2_non0 * dir.y * dir.z);
    result += coeffs[6] * (SH_FACTOR_COSINE_BAND2_0 * (3.0 * dir.z * dir.z - 1.0));
    result += coeffs[7] * (-SH_FACTOR_COSINE_BAND2_non0 * dir.x * dir.z);
    result += coeffs[8] * (SH_FACTOR_COSINE_BAND2_non0 * 0.5 * (dir.x * dir.x - dir.y * dir.y));

    return max(vec3f(0.0), result);
}