use crate::{
    primary_depth_buffer::PrimaryDepthBuffer,
    render_output::HdrBackbuffer,
    resource_managers::{
        GlobalBindings, PipelineError, PipelineManager, RenderPipelineDescriptor,
        RenderPipelineHandle, ShaderEntryPoint,
    },
    wgpu_utils::{BindGroupBuilder, BindGroupLayoutBuilder},
};

pub struct TerrainRenderer {
    render_pipeline: RenderPipelineHandle,
    bindgroup: wgpu::BindGroup,
}

impl TerrainRenderer {
    pub fn new(
        device: &wgpu::Device,
        global_bindings: &GlobalBindings,
        pipeline_manager: &mut PipelineManager,
    ) -> Result<Self, PipelineError> {
        let bindgroup_layout = BindGroupLayoutBuilder::new().create(device, "Terrain");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terrain"),
            bind_group_layouts: &[
                &global_bindings.bind_group_layout.layout,
                &bindgroup_layout.layout,
            ],
            push_constant_ranges: &[],
        });
        let render_pipeline = pipeline_manager.create_render_pipeline(
            device,
            RenderPipelineDescriptor {
                debug_label: "Terrain".to_owned(),
                layout: pipeline_layout,
                vertex_shader: ShaderEntryPoint::first_in("terrain.wgsl"),
                fragment_shader: ShaderEntryPoint::first_in("terrain.wgsl"),
                fragment_targets: vec![HdrBackbuffer::FORMAT.into()],
                primitive: wgpu::PrimitiveState {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    //polygon_mode: wgpu::PolygonMode::Line,
                    ..Default::default()
                },
                depth_stencil: Some(PrimaryDepthBuffer::STATE_WRITE),
                multisample: wgpu::MultisampleState::default(),
            },
        )?;

        let bindgroup = BindGroupBuilder::new(&bindgroup_layout).create(device, "Terrain");

        Ok(Self {
            render_pipeline,
            bindgroup,
        })
    }

    pub fn draw(
        &self,
        rpass: &mut wgpu::RenderPass<'_>,
        pipeline_manager: &PipelineManager,
    ) -> Result<(), PipelineError> {
        let pipeline = pipeline_manager.get_render_pipeline(self.render_pipeline)?;

        let grid_size = 100;
        let num_quads = grid_size * grid_size;
        let num_triangles = num_quads * 2;
        let num_vertices = num_triangles * 3;

        rpass.push_debug_group("Terrain");
        rpass.set_bind_group(1, &self.bindgroup, &[]);
        rpass.set_pipeline(pipeline);
        rpass.draw(0..num_vertices, 0..1);
        rpass.pop_debug_group();

        Ok(())
    }
}
