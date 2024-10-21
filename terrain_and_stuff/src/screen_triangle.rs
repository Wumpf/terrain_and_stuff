use crate::resource_managers::ShaderEntryPoint;

pub fn screen_triangle_vertex_shader() -> ShaderEntryPoint {
    ShaderEntryPoint {
        path: "screen_triangle.wgsl".into(),
        function_name: "vs_main".to_owned(),
    }
}
