// Transmittance LUT.
//
// Each pixel coordinate corresponds to a height and sun zenith angle.
// The value is the transmittance from that point to sun, through the atmosphere using single scattering only

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // TODO: Stuff!
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
