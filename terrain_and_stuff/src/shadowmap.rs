use crate::resource_managers::{
    GlobalBindings, PipelineError, PipelineManager, RenderPipelineDescriptor, RenderPipelineHandle,
    ShaderEntryPoint,
};
use crate::wgpu_utils::{BindGroupBuilder, BindGroupLayoutBuilder};

pub struct ShadowMap {
    texture_view: wgpu::TextureView,

    debug_bind_group: wgpu::BindGroup,
    debug_draw: RenderPipelineHandle,
}

const RESOLUTION: u32 = 2024;

impl ShadowMap {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub const STATE_WRITE: wgpu::DepthStencilState = wgpu::DepthStencilState {
        format: Self::FORMAT,
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::LessEqual,
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState::IGNORE,
            back: wgpu::StencilFaceState::IGNORE,
            read_mask: 0,
            write_mask: 0,
        },
        bias: wgpu::DepthBiasState {
            constant: 2,
            slope_scale: 2.0,
            clamp: 0.0,
        },
    };

    pub fn new(
        device: &wgpu::Device,
        global_bindings: &GlobalBindings,
        pipeline_manager: &mut PipelineManager,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, PipelineError> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadowmap"),
            size: wgpu::Extent3d {
                width: RESOLUTION,
                height: RESOLUTION,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::FORMAT],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let debug_bind_group_layout = BindGroupLayoutBuilder::new()
            .next_binding_fragment(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
            .create(device, "Debug shadow map");
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Debug shadow map"),
            bind_group_layouts: &[
                &global_bindings.bind_group_layout.layout,
                &debug_bind_group_layout.layout,
            ],
            push_constant_ranges: &[],
        });

        let debug_bind_group = BindGroupBuilder::new(&debug_bind_group_layout)
            .texture(&texture_view)
            .create(device, "Debug shadow map");

        let debug_draw = pipeline_manager.create_render_pipeline(
            device,
            RenderPipelineDescriptor {
                debug_label: "Debug shadow map".to_owned(),
                layout: pipeline_layout,
                vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                fragment_shader: Some(ShaderEntryPoint::first_in("debug_draw_shadow_map.wgsl")),
                fragment_targets: vec![surface_format.into()],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
        )?;

        Ok(Self {
            texture_view,
            debug_draw,
            debug_bind_group,
        })
    }

    /// Compute an orthographic shadow projection matrix that covers the world_bounding_box
    /// from the perspective of the sun (light_direction).
    pub fn shadow_projection_from_world(
        &self,
        light_dir: glam::Vec3,
        world_bounding_box: macaw::BoundingBox,
    ) -> glam::Mat4 {
        // Create light's basis (right, up, direction)
        // We use this as the light space - we don't need to "place" the camera for an orthographic projection,
        // since we can do that as part of the projection matrix.
        let light_space_from_world = {
            let tmp_up = if light_dir.abs().y < 0.99 {
                glam::Vec3::Y
            } else {
                glam::Vec3::X
            };
            let light_right = light_dir.cross(tmp_up).normalize();
            let light_up = light_right.cross(light_dir).normalize();
            glam::Mat3::from_cols(light_right, light_up, light_dir)
        };

        // Transform corners of the world bounding box into light space
        let light_space_bounding_box = macaw::BoundingBox::from_points(
            world_bounding_box
                .corners()
                .into_iter()
                .map(|corner| light_space_from_world * corner),
        );

        // This directly forms the projection matrix.
        let shadow_map_from_light_space = glam::Mat4::orthographic_rh(
            light_space_bounding_box.min.x,
            light_space_bounding_box.max.x,
            light_space_bounding_box.min.y,
            light_space_bounding_box.max.y,
            light_space_bounding_box.min.z,
            light_space_bounding_box.max.z,
        );

        shadow_map_from_light_space * glam::Mat4::from_mat3(light_space_from_world)
    }

    pub fn shadow_map_render_pass_descriptor(&self) -> wgpu::RenderPassDescriptor<'_> {
        wgpu::RenderPassDescriptor {
            label: Some("Shadowmap"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        }
    }

    pub fn debug_draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        pipeline_manager: &PipelineManager,
    ) -> Result<(), PipelineError> {
        render_pass.set_pipeline(pipeline_manager.get_render_pipeline(self.debug_draw)?);
        render_pass.set_bind_group(1, Some(&self.debug_bind_group), &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}
