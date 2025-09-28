struct Ray {
    origin: vec3f,
    direction: vec3f,
}


// Intersection of a ray with a sphere at the origin.
// Returns -1 if there is no intersection (or intersection is behind).
// Otherwise, returns distance on the ray to the closest intersection.
fn ray_sphere_intersect(ray: Ray, radius: f32) -> f32 {
    let sphere_origin_to_ray_origin = -ray.origin; // sphere_origin - ray_origin

    let b = dot(sphere_origin_to_ray_origin, ray.direction);

    let radius_sq = radius * radius;

    // A bit slower but more robust, see https://www.shadertoy.com/view/WdXfR2
    let fbd = b * ray.direction - sphere_origin_to_ray_origin;
    let discr = radius_sq - dot(fbd, fbd);

    if (discr < 0.0) {
        return -1.0;
    }
    let discr_sqrt = sqrt(discr);

    // Special case: inside sphere, use far discriminant
    if discr_sqrt > b {
        return b + discr_sqrt;
    }
    return b - discr_sqrt;
}