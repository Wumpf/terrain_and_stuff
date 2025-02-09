#import "global_bindings.wgsl"::{frame}

struct VertexOutput {
    // Mark output position as invariant so it's safe to use it with depth test Equal.
    // Without @invariant, different usages in different render pipelines might optimize differently,
    // causing slightly different results.
    @invariant @builtin(position)
    position: vec4f,
    @location(0)
    texcoord: vec2f,
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
    let quad_index = vertex_index / 6;
    let index_in_quad = vertex_index % 6;
    let grid_position = quad_positions[index_in_quad];

    let world_position = vec3f(
        f32(grid_position.x) * 100.0,
        -10.0,
        f32(grid_position.y) * 100.0,
    );

    var out: VertexOutput;
    out.position = frame.projection_from_world * vec4f(world_position, 1.0);
    out.texcoord = vec2f(grid_position);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0)  vec4f {
    return vec4f(in.texcoord, 0.0, 1.0);
}
