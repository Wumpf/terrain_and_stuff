#import "global_bindings.wgsl"::{frame}
#import "intersections.wgsl"::{Ray}

fn camera_ray_from_screenuv(texcoord: vec2f) -> Ray {
    return Ray(frame.camera_position, camera_dir_from_screenuv(texcoord));
}

fn texcoord_to_ndc(texcoord: vec2f) -> vec2f {
    return vec2f(texcoord.x - 0.5, 0.5 - texcoord.y) * 2.0;
}

// Returns the world space camera ray direction through a given screen uv coordinates (ranging from 0 to 1, i.e. NOT ndc coordinates)
fn camera_dir_from_screenuv(texcoord: vec2f) -> vec3f {
    let view_space_dir = camera_view_space_dir_from_screenuv(texcoord);

    // Note that since view_from_world is an orthonormal matrix, multiplying it from the right
    // means multiplying it with the transpose, meaning multiplying with the inverse!
    // (i.e. we get world_from_view for free as long as we only care about directions!)
    return (view_space_dir * frame.view_from_world).xyz;
}

fn camera_view_space_dir_from_screenuv(texcoord: vec2f) -> vec3f {
    let ndc = texcoord_to_ndc(texcoord);

    // Positive z since z dir is towards viewer (by RUF convention).
    let view_space_dir = vec3f(ndc * frame.tan_half_fov, 1.0);

    return normalize(view_space_dir);
}

fn view_space_depth_from_depth_buffer(depth_buffer_depth: f32) -> f32 {
    // We're using `glam::perspective_infinite_reverse_lh` which maps z = near to depth = 1 and z = infinity to depth = 0.
    // Making this trivial to invert!
    return frame.projection_from_view[3][2] / depth_buffer_depth;
}

fn view_space_position_from_depth_buffer(depth_buffer_depth: f32, texcoord: vec2f) -> vec3f {
    let d = view_space_depth_from_depth_buffer(depth_buffer_depth);
    let ndc = texcoord_to_ndc(texcoord);

    // Positive z since z dir is towards viewer (by RUF convention).
    return vec3f(ndc * frame.tan_half_fov * d, d);
}
