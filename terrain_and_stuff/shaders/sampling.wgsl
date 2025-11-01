import package::constants::{TAU, GOLDEN_RATIO};

// Fibbonaci lattice for point sampling on sphere.
// https://extremelearning.com.au/how-to-evenly-distribute-points-on-a-sphere-more-effectively-than-the-canonical-fibonacci-lattice/
fn uniform_sampled_sphere_theta_phi(index: u32, num_samples: u32) -> vec2f {
    let index_f = f32(index);
    let theta = TAU * index_f / GOLDEN_RATIO;
    let phi = acos(1.0 - 2.0 * (index_f + 0.5) / f32(num_samples));
    return vec2f(theta, phi);
}

// Samples a direction on the unit sphere.
//
// See `uniform_sampled_sphere_theta_phi` for details.
fn uniform_sampled_sphere_direction(index: u32, num_samples: u32) -> vec3f {
    let theta_phi = uniform_sampled_sphere_theta_phi(index, num_samples);
    let theta = theta_phi.x;
    let phi = theta_phi.y;

    let cosPhi = cos(phi);
    let sinPhi = sin(phi);
    let cosTheta = cos(theta);
    let sinTheta = sin(theta);
    return vec3f(sinPhi * sinTheta, cosPhi, sinPhi * cosTheta);
}
