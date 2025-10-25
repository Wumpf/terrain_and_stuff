struct FrameUniformBuffer {
    view_from_world: mat4x3f,
    projection_from_view: mat4x4f,
    projection_from_world: mat4x4f,

    shadow_map_from_world: mat4x4f,

    /// Camera position in world space.
    camera_position: vec3f,

    /// Camera direction in world space.
    /// Same as -vec3f(view_from_world[0].z, view_from_world[1].z, view_from_world[2].z)
    camera_forward: vec3f,

    /// (tan(fov_y / 2) * aspect_ratio, tan(fov_y /2)), i.e. half ratio of screen dimension to screen distance in x & y.
    /// Both values are set to f32max for orthographic projection
    tan_half_fov: vec2f,

    /// Normalized direction to the sun/moon in world space.
    dir_to_sun: vec3f,
};

@group(0) @binding(0)
var<uniform> frame_uniforms: FrameUniformBuffer;

@group(0) @binding(1)
var bluenoise: texture_2d<f32>;

@group(0) @binding(2)
var nearest_sampler_clamp: sampler;
@group(0) @binding(3)
var nearest_sampler_repeat: sampler;

@group(0) @binding(4)
var trilinear_sampler_clamp: sampler;
@group(0) @binding(5)
var trilinear_sampler_repeat: sampler;