use std::num::NonZeroU64;

use wgpu::util::DeviceExt as _;

use crate::{
    primary_depth_buffer::PrimaryDepthBuffer,
    render_output::HdrBackbuffer,
    resource_managers::{
        GlobalBindings, PipelineError, PipelineManager, RenderPipelineDescriptor,
        RenderPipelineHandle, ShaderEntryPoint,
    },
    shadowmap::ShadowMap,
    wgpu_utils::{BindGroupBuilder, BindGroupLayoutBuilder},
};

pub struct TerrainRenderer {
    render_pipeline: RenderPipelineHandle,
    shadow_map_pipeline: RenderPipelineHandle,

    bindgroup: wgpu::BindGroup,
    texture_size: glam::UVec2,
}

impl TerrainRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        global_bindings: &GlobalBindings,
        sky_and_sun_lighting_params_buffer: &wgpu::Buffer,
        pipeline_manager: &mut PipelineManager,
    ) -> Result<Self, PipelineError> {
        // Hardcoded heightmap for now. Want to generate eventually!
        let heightmap_texture = {
            let heightmap_tiff = include_bytes!("../../../assets/heightmap.tif");
            let mut decoder =
                tiff::decoder::Decoder::new(std::io::Cursor::new(heightmap_tiff)).unwrap();
            let size = decoder.dimensions().unwrap();
            let mut image = decoder.read_image().unwrap();

            let tiff::decoder::DecodingBuffer::F32(image_buffer) = image.as_buffer(0) else {
                panic!("Heightmap is not a float buffer");
            };

            device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("Heightmap"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[wgpu::TextureFormat::R32Float],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                bytemuck::cast_slice(image_buffer),
            )
        };

        let bindgroup_layout = BindGroupLayoutBuilder::new()
            // Heightmap.
            .next_binding_vertex(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
            // Sun color + SH coefficients.
            .next_binding_fragment(wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(sky_and_sun_lighting_params_buffer.size()),
            })
            .create(device, "Terrain");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terrain"),
            bind_group_layouts: &[
                &global_bindings.bind_group_layout.layout,
                &bindgroup_layout.layout,
            ],
            push_constant_ranges: &[],
        });

        let render_pipeline_descriptor = RenderPipelineDescriptor {
            debug_label: "Terrain".to_owned(),
            layout: pipeline_layout,
            vertex_shader: ShaderEntryPoint::first_in("terrain.wgsl"),
            fragment_shader: Some(ShaderEntryPoint::first_in("terrain.wgsl")),
            fragment_targets: vec![HdrBackbuffer::FORMAT.into()],
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(PrimaryDepthBuffer::STATE_WRITE),
            multisample: wgpu::MultisampleState::default(),
        };
        let render_pipeline =
            pipeline_manager.create_render_pipeline(device, render_pipeline_descriptor.clone())?;

        let shadow_map_pipeline = pipeline_manager.create_render_pipeline(
            device,
            RenderPipelineDescriptor {
                debug_label: "Terrain Shadow".to_owned(),
                vertex_shader: render_pipeline_descriptor
                    .vertex_shader
                    .with_feature("SHADOW_MAP"),
                fragment_shader: None,
                fragment_targets: Vec::new(),
                depth_stencil: Some(ShadowMap::STATE_WRITE),
                ..render_pipeline_descriptor
            },
        )?;

        let bindgroup = BindGroupBuilder::new(&bindgroup_layout)
            .texture(&heightmap_texture.create_view(&wgpu::TextureViewDescriptor::default()))
            .buffer(sky_and_sun_lighting_params_buffer.as_entire_buffer_binding())
            .create(device, "Terrain");

        Ok(Self {
            render_pipeline,
            shadow_map_pipeline,
            bindgroup,
            texture_size: glam::uvec2(heightmap_texture.width(), heightmap_texture.height()),
        })
    }

    pub fn bounding_box(&self) -> macaw::BoundingBox {
        const HEIGHT_SCALE_FACTOR: f32 = 15000.0;
        const GRID_TO_WORLD: f32 = 6.0;

        macaw::BoundingBox::from_min_max(
            glam::Vec3::ZERO,
            glam::vec3(
                GRID_TO_WORLD * self.texture_size.x as f32,
                HEIGHT_SCALE_FACTOR,
                GRID_TO_WORLD * self.texture_size.y as f32,
            ),
        )
    }

    pub fn draw_shadow_map(
        &self,
        rpass: &mut wgpu::RenderPass<'_>,
        pipeline_manager: &PipelineManager,
    ) -> Result<(), PipelineError> {
        let pipeline = pipeline_manager.get_render_pipeline(self.shadow_map_pipeline)?;
        self.draw_impl(rpass, pipeline)
    }

    pub fn draw(
        &self,
        rpass: &mut wgpu::RenderPass<'_>,
        pipeline_manager: &PipelineManager,
    ) -> Result<(), PipelineError> {
        let pipeline = pipeline_manager.get_render_pipeline(self.render_pipeline)?;
        self.draw_impl(rpass, pipeline)
    }

    fn draw_impl(
        &self,
        rpass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
    ) -> Result<(), PipelineError> {
        let num_quads = self.texture_size.x * self.texture_size.y;
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
