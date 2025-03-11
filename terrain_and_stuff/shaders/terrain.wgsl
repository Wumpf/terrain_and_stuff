#import "global_bindings.wgsl"::{frame_uniforms}
#import "atmosphere/sky_and_sun_lighting.wgsl"::{SkyAndSunLightingParams}
#import "sh.wgsl"::{evaluate_sh2_cosine}

@group(1) @binding(0) var heightmap: texture_2d<f32>;
@group(1) @binding(1) var<uniform> sky_and_sun_lighting_params: SkyAndSunLightingParams;

struct VertexOutput {
    // Mark output position as invariant so it's safe to use it with depth test Equal.
    // Without @invariant, different usages in different render pipelines might optimize differently,
    // causing slightly different results.
    @invariant @builtin(position)
    position: vec4f,
    @location(0)
    texcoord: vec2f,
    @location(1)
    normal: vec3f,
};

// 5/0    1
//  x ----x
//  | \   |
//  |  \  |
//  |   \ |
//  x ----x
//  4    3/2
var<private> quad_positions: array<vec2i, 6> = array<vec2i, 6>(
    vec2i(0, 0),
    vec2i(1, 0),
    vec2i(1, 1),
    vec2i(1, 1),
    vec2i(0, 1),
    vec2i(0, 0),
);

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let grid_size = 4096;
    let grid_size_f = f32(grid_size);

    let quad_index = i32(vertex_index / 6);
    let quad_coord = vec2i(quad_index % grid_size, quad_index / grid_size);

    let index_in_quad = i32(vertex_index % 6);
    let grid_position = quad_positions[index_in_quad];

    let grid_to_world = 6.0;
    let height_scale_factor = 15000.0;

    let plane_position = grid_position + quad_coord;
    let height = textureLoad(heightmap, plane_position, 0).r * height_scale_factor;

    // Normal via central difference
    let normal = normalize(vec3f(
        (textureLoad(heightmap, plane_position - vec2i(1, 0), 0).r -
        textureLoad(heightmap, plane_position + vec2i(1, 0), 0).r) * height_scale_factor,
        2.0,
        (textureLoad(heightmap, plane_position - vec2i(0, 1), 0).r -
        textureLoad(heightmap, plane_position + vec2i(0, 1), 0).r) * height_scale_factor,
    ));

    let world_position_2d = vec2f(plane_position) * grid_to_world;
    let world_position = vec3f(world_position_2d, height).xzy;

    var out: VertexOutput;
    out.position = frame_uniforms.projection_from_world * vec4f(world_position, 1.0);
    out.texcoord = world_position.xz / (grid_size_f * grid_to_world);
    out.normal = normal;
    return out;
}


@fragment
fn fs_main(in: VertexOutput) -> @location(0)  vec4f {
    let normal = normalize(in.normal);

    let illuminance_sky = evaluate_sh2_cosine(normal, sky_and_sun_lighting_params.sky_luminance_sh_coefficients);
    let illuminance_direct = sky_and_sun_lighting_params.sun_illuminance * max(dot(normal, frame_uniforms.dir_to_sun), 0.0);
    let illuminance = illuminance_direct + illuminance_sky;

    // TODO: the sky illuminance doesn't look quite right yet. let's add ui accessible debug flags.

    return vec4f(illuminance, 1.0);

    // DEBUG:
   // return vec4f(normal * 0.5 + 0.5, 1.0);
}
