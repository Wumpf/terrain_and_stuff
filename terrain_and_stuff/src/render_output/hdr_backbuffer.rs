use crate::{
    resource_managers::{
        PipelineError, PipelineManager, RenderPipelineDescriptor, RenderPipelineHandle,
        ShaderEntryPoint,
    },
    wgpu_utils::{BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutWithDesc},
};

/// Defines the linear HDR backbuffer and display transform to an LDR surface.
///
/// Assumes HDR Rec.709/sRGB in optical units (no OETF) and applies OETF as part of the display transform.
/// (no HDR screen support yet)
pub struct HdrBackbuffer {
    hdr_backbuffer: wgpu::Texture,
    hdr_backbuffer_view: wgpu::TextureView,

    bind_group_layout: BindGroupLayoutWithDesc,
    bind_group: wgpu::BindGroup,
    display_transform_pipeline: RenderPipelineHandle,
}

impl HdrBackbuffer {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(
        device: &wgpu::Device,
        resolution: glam::UVec2,
        pipeline_manager: &mut PipelineManager,
        output_format: wgpu::TextureFormat,
    ) -> Result<Self, PipelineError> {
        let bind_group_layout = BindGroupLayoutBuilder::new()
            .next_binding_fragment(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
            .create(device, "Read HDR Backbuffer");
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Display transform"),
            bind_group_layouts: &[&bind_group_layout.layout],
            push_constant_ranges: &[],
        });

        let (hdr_backbuffer, hdr_backbuffer_view, bind_group) =
            Self::crate_backbuffer_texture(device, resolution, &bind_group_layout);

        let display_transform_pipeline = pipeline_manager.create_render_pipeline(
            device,
            RenderPipelineDescriptor {
                debug_label: "Display transform".to_owned(),
                layout: pipeline_layout,
                vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                fragment_shader: ShaderEntryPoint::first_in("display_transform.wgsl"),
                fragment_targets: vec![output_format.into()],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
        )?;

        Ok(HdrBackbuffer {
            hdr_backbuffer,
            hdr_backbuffer_view,

            bind_group_layout,
            bind_group,
            display_transform_pipeline,
        })
    }

    fn crate_backbuffer_texture(
        device: &wgpu::Device,
        resolution: glam::UVec2,
        bind_group_layout: &BindGroupLayoutWithDesc,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
        let size = wgpu::Extent3d {
            width: resolution.x,
            height: resolution.y,
            depth_or_array_layers: 1,
        };
        let hdr_backbuffer = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HdrBackbuffer"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::FORMAT],
        });
        let hdr_backbuffer_view = hdr_backbuffer.create_view(&Default::default());
        let bind_group = BindGroupBuilder::new(bind_group_layout)
            .texture(&hdr_backbuffer_view)
            .create(device, "Display transform");

        (hdr_backbuffer, hdr_backbuffer_view, bind_group)
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.hdr_backbuffer_view
    }

    pub fn on_resize(&mut self, device: &wgpu::Device, new_resolution: glam::UVec2) {
        let (hdr_backbuffer, hdr_backbuffer_view, bind_group) =
            Self::crate_backbuffer_texture(device, new_resolution, &self.bind_group_layout);

        self.hdr_backbuffer = hdr_backbuffer;
        self.hdr_backbuffer_view = hdr_backbuffer_view;
        self.bind_group = bind_group;
    }

    pub fn display_transform(
        &self,
        target: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        pipeline_manager: &PipelineManager,
    ) -> Option<()> {
        // TODO: All this tonemapping does is go from half (linear) to srgb. Do some nice tonemapping here!
        // Note that we can't use a compute shader here since that would require STORAGE usage flag on the final output which we can't do since it's srgb!
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Display transform"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None, // TODO: wgpu_profiler!
            occlusion_query_set: None,
        });

        render_pass
            .set_pipeline(pipeline_manager.get_render_pipeline(self.display_transform_pipeline)?);
        render_pass.set_bind_group(0, Some(&self.bind_group), &[]);
        render_pass.draw(0..3, 0..1);

        Some(())
    }
}
