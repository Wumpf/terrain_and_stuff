use crate::{
    EncoderScope,
    primary_depth_buffer::PrimaryDepthBuffer,
    render_output::HdrBackbuffer,
    resource_managers::{
        ComputePipelineDescriptor, ComputePipelineHandle, GlobalBindings, PipelineError,
        PipelineManager, RenderPipelineDescriptor, RenderPipelineHandle, ShaderEntryPoint,
    },
    wgpu_utils::{BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutWithDesc},
};

#[derive(Debug)]
pub struct AtmosphereParams {
    /// Sun's azimuth angle in radians.
    pub sun_azimuth: f32,
    /// Sun's altitude angle in radians.
    pub sun_altitude: f32,
}

impl Default for AtmosphereParams {
    fn default() -> Self {
        Self {
            sun_azimuth: std::f32::consts::PI,
            sun_altitude: std::f32::consts::PI / 4.0,
        }
    }
}

impl AtmosphereParams {
    pub fn dir_to_sun(&self) -> glam::Vec3 {
        let (sin_altitude, cos_altitude) = self.sun_altitude.sin_cos();
        let (sin_azimuth, cos_azimuth) = self.sun_azimuth.sin_cos();
        glam::vec3(
            cos_altitude * cos_azimuth,
            sin_altitude,
            cos_altitude * sin_azimuth,
        )
    }
}

pub struct Atmosphere {
    render_pipe_transmittance_lut: RenderPipelineHandle,
    render_pipe_raymarch_sky: RenderPipelineHandle,

    compute_pipe_sh: ComputePipelineHandle,

    raymarch_bindgroup_layout: BindGroupLayoutWithDesc,
    raymarch_bindgroup: wgpu::BindGroup,

    transmittance_lut: wgpu::TextureView,

    pub parameters: AtmosphereParams,
}

impl Atmosphere {
    const TRANSMITTANCE_LUT_SIZE: wgpu::Extent3d = wgpu::Extent3d {
        width: 256,
        height: 64,
        depth_or_array_layers: 1,
    };

    pub fn new(
        device: &wgpu::Device,
        global_bindings: &GlobalBindings,
        pipeline_manager: &mut PipelineManager,
        primary_depth_buffer: &PrimaryDepthBuffer,
    ) -> Result<Self, PipelineError> {
        // Transmittance.
        let (transmittance_lut, render_pipe_transmittance_lut, raymarch_bindgroup_layout) = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("EmptyLayout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

            let transmittance_lut = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Transmittance LUT"),
                size: Self::TRANSMITTANCE_LUT_SIZE,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let transmittance_lut =
                transmittance_lut.create_view(&wgpu::TextureViewDescriptor::default());

            let render_pipe_transmittance_lut = pipeline_manager.create_render_pipeline(
                device,
                RenderPipelineDescriptor {
                    debug_label: "atmosphere/transmittance_lut".to_owned(),
                    layout,
                    vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                    fragment_shader: ShaderEntryPoint::first_in(
                        "atmosphere/transmittance_lut.wgsl",
                    ),
                    fragment_targets: vec![HdrBackbuffer::FORMAT.into()],
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                },
            )?;

            let raymarch_bindings = BindGroupLayoutBuilder::new()
                .next_binding_fragment(wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                })
                .next_binding_fragment(wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                })
                .create(device, "atmosphere/raymarch_sky");

            (
                transmittance_lut,
                render_pipe_transmittance_lut,
                raymarch_bindings,
            )
        };

        // Raymarch.
        let render_pipe_raymarch_sky = {
            let raymarch_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("RaymarchLayout"),
                bind_group_layouts: &[
                    &global_bindings.bind_group_layout.layout,
                    &raymarch_bindgroup_layout.layout,
                ],
                push_constant_ranges: &[],
            });
            pipeline_manager.create_render_pipeline(
                device,
                RenderPipelineDescriptor {
                    debug_label: "atmosphere/raymarch_sky".to_owned(),
                    layout: raymarch_layout,
                    vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                    fragment_shader: ShaderEntryPoint::first_in("atmosphere/raymarch_sky.wgsl"),
                    fragment_targets: vec![wgpu::ColorTargetState {
                        format: HdrBackbuffer::FORMAT,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                // Use dual source blending:
                                // color = src0 + src1 * dst
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::Src1,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                },
            )?
        };
        let raymarch_bindgroup = Self::create_raymarch_bindgroup(
            device,
            &raymarch_bindgroup_layout,
            &transmittance_lut,
            primary_depth_buffer,
        );

        // Compute pipeline for computing SH coefficients.
        let compute_pipe_sh = {
            let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compute sh layout"),
                bind_group_layouts: &[&global_bindings.bind_group_layout.layout],
                push_constant_ranges: &[],
            });

            pipeline_manager.create_compute_pipeline(
                device,
                ComputePipelineDescriptor {
                    debug_label: "atmosphere/sh".to_owned(),
                    layout: compute_layout,
                    compute_shader: ShaderEntryPoint::first_in("atmosphere/compute_sh.wgsl"),
                },
            )?
        };

        Ok(Self {
            render_pipe_transmittance_lut,
            render_pipe_raymarch_sky,
            compute_pipe_sh,
            raymarch_bindgroup_layout,
            raymarch_bindgroup,
            transmittance_lut,
            parameters: AtmosphereParams::default(),
        })
    }

    fn create_raymarch_bindgroup(
        device: &wgpu::Device,
        raymarch_bindings: &BindGroupLayoutWithDesc,
        transmittance_lut: &wgpu::TextureView,
        primary_depth_buffer: &PrimaryDepthBuffer,
    ) -> wgpu::BindGroup {
        BindGroupBuilder::new(raymarch_bindings)
            .texture(transmittance_lut)
            .texture(primary_depth_buffer.view())
            .create(device, "atmosphere/raymarch_sky")
    }

    pub fn on_resize(&mut self, device: &wgpu::Device, primary_depth_buffer: &PrimaryDepthBuffer) {
        self.raymarch_bindgroup = Self::create_raymarch_bindgroup(
            device,
            &self.raymarch_bindgroup_layout,
            &self.transmittance_lut,
            primary_depth_buffer,
        );
    }

    pub fn prepare(
        &self,
        encoder: &mut EncoderScope<'_>,
        pipeline_manager: &PipelineManager,
        global_bindings: &GlobalBindings,
    ) -> Result<(), PipelineError> {
        let mut encoder = encoder.scope("Prepare atmosphere");
        {
            let mut compute_pass = encoder.scoped_compute_pass("SH coefficients");
            compute_pass.set_pipeline(pipeline_manager.get_compute_pipeline(self.compute_pipe_sh)?);
            compute_pass.set_bind_group(0, &global_bindings.bind_group, &[]);
            compute_pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let mut render_pass = encoder.scoped_render_pass(
                "Transmittance LUT",
                wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.transmittance_lut,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                },
            );

            render_pass.set_pipeline(
                pipeline_manager.get_render_pipeline(self.render_pipe_transmittance_lut)?,
            );
            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }

    pub fn draw(
        &self,
        rpass: &mut wgpu::RenderPass<'_>,
        pipeline_manager: &PipelineManager,
    ) -> Result<(), PipelineError> {
        let pipeline = pipeline_manager.get_render_pipeline(self.render_pipe_raymarch_sky)?;

        rpass.push_debug_group("Raymarch Atmosphere");
        rpass.set_bind_group(1, &self.raymarch_bindgroup, &[]);
        rpass.set_pipeline(pipeline);
        rpass.draw(0..3, 0..1);
        rpass.pop_debug_group();

        Ok(())
    }
}
