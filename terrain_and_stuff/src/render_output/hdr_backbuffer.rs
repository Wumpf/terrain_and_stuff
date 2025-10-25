use wgpu::util::DeviceExt;

use crate::{
    resource_managers::{
        GlobalBindings, PipelineError, PipelineManager, RenderPipelineDescriptor,
        RenderPipelineHandle, ShaderEntryPoint,
    },
    wgpu_utils::{BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutWithDesc},
};

/// Defines the linear HDR backbuffer and display transform to an LDR surface.
///
/// Assumes HDR Rec.709/sRGB in optical units (no OETF) and applies OETF as part of the display transform.
/// (no HDR screen support yet)
pub struct HdrBackbuffer {
    hdr_backbuffer_view: wgpu::TextureView,
    tony_lut_view: wgpu::TextureView,

    bind_group_layout: BindGroupLayoutWithDesc,
    bind_group: wgpu::BindGroup,
    display_transform_pipeline: RenderPipelineHandle,
}

impl HdrBackbuffer {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        global_bindings: &GlobalBindings,
        resolution: glam::UVec2,
        pipeline_manager: &mut PipelineManager,
        output_format: wgpu::TextureFormat,
    ) -> Result<Self, PipelineError> {
        let tony_lut_view = load_tony_lut(device, queue);

        let bind_group_layout = BindGroupLayoutBuilder::new()
            .next_binding_fragment(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
            .next_binding_fragment(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D3,
                multisampled: false,
            })
            .create(device, "Read HDR Backbuffer");
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Display transform"),
            bind_group_layouts: &[
                &global_bindings.bind_group_layout.layout,
                &bind_group_layout.layout,
            ],
            push_constant_ranges: &[],
        });

        let (hdr_backbuffer_view, bind_group) = Self::create_backbuffer_and_bindgroup(
            device,
            resolution,
            &bind_group_layout,
            &tony_lut_view,
        );

        let display_transform_pipeline = pipeline_manager.create_render_pipeline(
            device,
            RenderPipelineDescriptor {
                debug_label: "Display transform".to_owned(),
                layout: pipeline_layout,
                vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                fragment_shader: Some(ShaderEntryPoint::first_in("display_transform.wgsl")),
                fragment_targets: vec![output_format.into()],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
        )?;

        Ok(HdrBackbuffer {
            hdr_backbuffer_view,
            tony_lut_view,

            bind_group_layout,
            bind_group,
            display_transform_pipeline,
        })
    }

    fn create_backbuffer_and_bindgroup(
        device: &wgpu::Device,
        resolution: glam::UVec2,
        bind_group_layout: &BindGroupLayoutWithDesc,
        tony_lut_view: &wgpu::TextureView,
    ) -> (wgpu::TextureView, wgpu::BindGroup) {
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
            .texture(tony_lut_view)
            .create(device, "Display transform");

        (hdr_backbuffer_view, bind_group)
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.hdr_backbuffer_view
    }

    pub fn on_resize(&mut self, device: &wgpu::Device, new_resolution: glam::UVec2) {
        let (hdr_backbuffer_view, bind_group) = Self::create_backbuffer_and_bindgroup(
            device,
            new_resolution,
            &self.bind_group_layout,
            &self.tony_lut_view,
        );

        self.hdr_backbuffer_view = hdr_backbuffer_view;
        self.bind_group = bind_group;
    }

    pub fn display_transform(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        pipeline_manager: &PipelineManager,
        global_bindings: &GlobalBindings,
    ) -> Result<(), PipelineError> {
        render_pass
            .set_pipeline(pipeline_manager.get_render_pipeline(self.display_transform_pipeline)?);
        render_pass.set_bind_group(0, Some(&global_bindings.bind_group), &[]);
        render_pass.set_bind_group(1, Some(&self.bind_group), &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }

    pub fn color_attachment(&self) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: &self.hdr_backbuffer_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        }
    }
}

fn load_tony_lut(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let tony_lut_dds_bytes = include_bytes!("../../../assets/tony/lut.dds");
    let tony_lut = ddsfile::Dds::read(std::io::Cursor::new(tony_lut_dds_bytes))
        .expect("Failed reading embedded display transform LUT.");

    debug_assert_eq!(
        tony_lut.get_dxgi_format(),
        Some(ddsfile::DxgiFormat::R9G9B9E5_SharedExp)
    );
    debug_assert_eq!(tony_lut.get_width(), 48);
    debug_assert_eq!(tony_lut.get_height(), 48);
    debug_assert_eq!(tony_lut.get_depth(), 48);
    debug_assert_eq!(tony_lut.get_pitch(), Some(48 * 4));

    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Tony McMapface LUT"),
                size: wgpu::Extent3d {
                    width: 48,
                    height: 48,
                    depth_or_array_layers: 48,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: wgpu::TextureFormat::Rgb9e5Ufloat,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[wgpu::TextureFormat::Rgb9e5Ufloat],
            },
            wgpu::util::TextureDataOrder::LayerMajor, // Doesn't matter, no mipmaps!
            bytemuck::cast_slice(tony_lut.get_data(0).unwrap()),
        )
        .create_view(&Default::default())
}
