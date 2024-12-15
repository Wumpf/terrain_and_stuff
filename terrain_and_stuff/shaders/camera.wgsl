#import "global_bindings.wgsl"::{frame}
#import "intersections.wgsl"::{Ray}

fn camera_ray_from_screenuv(texcoord: vec2f) -> Ray {
    return Ray(frame.camera_position, camera_ray_direction_from_screenuv(texcoord));
}

// Returns the camera ray direction through a given screen uv coordinates (ranging from 0 to 1, i.e. NOT ndc coordinates)
fn camera_ray_direction_from_screenuv(texcoord: vec2f) -> vec3f {
    // convert [0, 1] to [-1, +1 (Normalized Device Coordinates)
    let ndc = vec2f(texcoord.x - 0.5, 0.5 - texcoord.y) * 2.0;

    // Positive z since z dir is towards viewer (by RUF convention).
    // TODO: Why -1 ndc? sounds like something else in the camera setup might be wrong.
    let view_space_dir = vec3f(-ndc * frame.tan_half_fov, 1.0);

    // Note that since view_from_world is an orthonormal matrix, multiplying it from the right
    // means multiplying it with the transpose, meaning multiplying with the inverse!
    // (i.e. we get world_from_view for free as long as we only care about directions!)
    let world_space_dir = (view_space_dir * frame.view_from_world).xyz;

    return normalize(world_space_dir);
}

