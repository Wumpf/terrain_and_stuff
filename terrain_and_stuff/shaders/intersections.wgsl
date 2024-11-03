struct Ray {
    origin: vec3f,
    direction: vec3f,
}


// Intersection of a ray with a sphere at the origin.
// Returns -1 if there is no intersection (or intersection is behind).
// Otherwise, returns distance on the ray to the closest intersection.
fn ray_sphere_intersect(ray: Ray, radius: f32) -> f32 {
    let b = dot(ray.origin, ray.direction); 
    let c = dot(ray.origin, ray.origin) - radius * radius;
    let discr = b*b - c;
    if (discr < 0.0) {
        return -1.0;
    }
    let discr_sqrt = sqrt(discr);

    // Special case: inside sphere, use far discriminant
    if discr_sqrt > b {
        return -b + discr_sqrt;
    }
    return -b - discr_sqrt;
}