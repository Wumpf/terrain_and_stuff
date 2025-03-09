//! Compute shader for computing luminace SH coefficients for the atmosphere at a fixed altitude.

@group(1) @binding(0) var<storage, read_write> sh_coefficients: array<vec3f>;

@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    sh_coefficients[0] = vec3f(0.0, 1.0, 2.0);
    sh_coefficients[1] = vec3f(3.0, 4.0, 5.0);
    sh_coefficients[2] = vec3f(6.0, 7.0, 8.0);
    sh_coefficients[3] = vec3f(9.0, 10.0, 11.0);
    sh_coefficients[4] = vec3f(12.0, 13.0, 14.0);
    sh_coefficients[5] = vec3f(15.0, 16.0, 17.0);
    sh_coefficients[6] = vec3f(18.0, 19.0, 20.0);
}
