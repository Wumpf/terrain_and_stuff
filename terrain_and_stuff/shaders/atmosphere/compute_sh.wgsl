//! Compute shader for computing luminace SH coefficients for the atmosphere at a fixed altitude.

//@group(0) @binding(0) var<storage, read_write> data: array<f32>;

@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    // TODO: stuff!
}