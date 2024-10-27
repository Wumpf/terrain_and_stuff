use crate::{
    render_output::HdrBackbuffer,
    resource_managers::{
        PipelineError, PipelineManager, RenderPipelineDescriptor, RenderPipelineHandle,
        ShaderEntryPoint,
    },
};

pub struct Sky {
    render_pipeline: RenderPipelineHandle,
}

impl Sky {
    pub fn new(
        device: &wgpu::Device,
        pipeline_manager: &mut PipelineManager,
    ) -> Result<Self, PipelineError> {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sky"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = pipeline_manager.create_render_pipeline(
            device,
            RenderPipelineDescriptor {
                debug_label: "Sky".to_owned(),
                layout,
                vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                fragment_shader: ShaderEntryPoint::first_in("sky.wgsl"),
                fragment_targets: vec![HdrBackbuffer::FORMAT.into()],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None, // TODO: make it possible to draw the sky last
                multisample: wgpu::MultisampleState::default(),
            },
        )?;

        Ok(Self { render_pipeline })
    }

    pub fn draw(
        &self,
        rpass: &mut wgpu::RenderPass<'_>,
        pipeline_manager: &PipelineManager,
    ) -> Option<()> {
        let pipeline = pipeline_manager.get_render_pipeline(self.render_pipeline)?;
        rpass.set_pipeline(pipeline);
        rpass.draw(0..3, 0..1);

        Some(())
    }
}
