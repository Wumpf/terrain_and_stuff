use std::num::NonZeroU64;

use wgpu::util::DeviceExt;

use crate::{
    EncoderScope,
    primary_depth_buffer::PrimaryDepthBuffer,
    render_output::HdrBackbuffer,
    resource_managers::{
        ComputePipelineDescriptor, ComputePipelineHandle, GlobalBindings, PipelineError,
        PipelineManager, RenderPipelineDescriptor, RenderPipelineHandle, ShaderEntryPoint,
    },
    wgpu_utils::{
        BindGroupBuilder, BindGroupLayoutBuilder, BindGroupLayoutWithDesc,
        wgpu_buffer_types::Vec3RowPadded,
    },
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
    render_pipe_render_atmosphere: RenderPipelineHandle,

    compute_pipe_sh: ComputePipelineHandle,

    raymarch_bindgroup_layout: BindGroupLayoutWithDesc,
    raymarch_bindgroup: wgpu::BindGroup,
    compute_sh_bind_group: wgpu::BindGroup,

    transmittance_lut: wgpu::TextureView,
    sh_coefficients: wgpu::Buffer,

    pub parameters: AtmosphereParams,
}

const TRANSMITTANCE_LUT_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 256,
    height: 64,
    depth_or_array_layers: 1,
};

const NUM_SH_SAMPLES: u32 = 1024;

impl Atmosphere {
    pub fn new(
        device: &wgpu::Device,
        global_bindings: &GlobalBindings,
        pipeline_manager: &mut PipelineManager,
        primary_depth_buffer: &PrimaryDepthBuffer,
    ) -> Result<Self, PipelineError> {
        let sh_coefficients_buffer_size = (1 + 3 + 5) * // SH bands 0, 1, 2
            (std::mem::size_of::<Vec3RowPadded>() as u64); // RGB for each band, need to add padding
        let sh_coefficients = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SH coefficients"),
            size: sh_coefficients_buffer_size,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Transmittance.
        let (transmittance_lut, render_pipe_transmittance_lut, raymarch_bindgroup_layout) = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("EmptyLayout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

            let transmittance_lut = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Transmittance LUT"),
                size: TRANSMITTANCE_LUT_SIZE,
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
                // [in] transmittance lut
                .next_binding_fragment(wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                })
                // [in] depth buffer
                .next_binding_fragment(wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                })
                // [in] sh coefficients (debugging only)
                .next_binding_fragment(wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(sh_coefficients_buffer_size),
                })
                .create(device, "atmosphere/render_atmosphere");

            (
                transmittance_lut,
                render_pipe_transmittance_lut,
                raymarch_bindings,
            )
        };

        // Render atmosphere.
        let render_pipe_render_atmosphere = {
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
                    debug_label: "atmosphere/render_atmosphere".to_owned(),
                    layout: raymarch_layout,
                    vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                    fragment_shader: ShaderEntryPoint::first_in(
                        "atmosphere/render_atmosphere.wgsl",
                    ),
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
            &sh_coefficients,
        );

        // Compute pipeline for computing SH coefficients.
        let (compute_pipe_sh, compute_sh_bind_group) = {
            let sampling_directions = generate_sampling_directions(NUM_SH_SAMPLES);
            let sampling_directions_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Sampling directions"),
                    contents: bytemuck::cast_slice(sampling_directions.as_slice()),
                    usage: wgpu::BufferUsages::STORAGE,
                });

            let bindings = BindGroupLayoutBuilder::new()
                // [in] transmittance lut
                .next_binding_compute(wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                })
                // [in] sampling directions
                .next_binding_compute(wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(sampling_directions_buffer.size()),
                })
                // [out] sh coefficients
                .next_binding_compute(wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(sh_coefficients_buffer_size),
                })
                .create(device, "atmosphere/sh");

            let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compute sh layout"),
                bind_group_layouts: &[&global_bindings.bind_group_layout.layout, &bindings.layout],
                push_constant_ranges: &[],
            });

            let pipeline = pipeline_manager.create_compute_pipeline(
                device,
                ComputePipelineDescriptor {
                    debug_label: "atmosphere/sh".to_owned(),
                    layout: compute_layout,
                    // TODO: naga_oil doesn't support override constants, it has its own preprocessor, but then have to do this at shader load
                    // compute_shader: ShaderEntryPoint {
                    //     path: "atmosphere/compute_sh.wgsl".into(),
                    //     function_name: None,
                    //     overrides: vec![("NUM_SAMPLES", NUM_SH_SAMPLES as f64)],
                    // },
                    compute_shader: ShaderEntryPoint::first_in("atmosphere/compute_sh.wgsl"),
                },
            )?;

            let compute_sh_bind_group = BindGroupBuilder::new(&bindings)
                .texture(&transmittance_lut)
                .buffer(sampling_directions_buffer.as_entire_buffer_binding())
                .buffer(sh_coefficients.as_entire_buffer_binding())
                .create(device, "atmosphere/compute_sh");

            (pipeline, compute_sh_bind_group)
        };

        Ok(Self {
            render_pipe_transmittance_lut,
            render_pipe_render_atmosphere,
            compute_pipe_sh,
            raymarch_bindgroup_layout,
            raymarch_bindgroup,
            compute_sh_bind_group,
            sh_coefficients,
            transmittance_lut,
            parameters: AtmosphereParams::default(),
        })
    }

    fn create_raymarch_bindgroup(
        device: &wgpu::Device,
        raymarch_bindings: &BindGroupLayoutWithDesc,
        transmittance_lut: &wgpu::TextureView,
        primary_depth_buffer: &PrimaryDepthBuffer,
        sh_coefficients: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        BindGroupBuilder::new(raymarch_bindings)
            .texture(transmittance_lut)
            .texture(primary_depth_buffer.view())
            .buffer(sh_coefficients.as_entire_buffer_binding())
            .create(device, "atmosphere/render_atmosphere")
    }

    pub fn on_resize(&mut self, device: &wgpu::Device, primary_depth_buffer: &PrimaryDepthBuffer) {
        self.raymarch_bindgroup = Self::create_raymarch_bindgroup(
            device,
            &self.raymarch_bindgroup_layout,
            &self.transmittance_lut,
            primary_depth_buffer,
            &self.sh_coefficients,
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
            compute_pass.set_bind_group(1, &self.compute_sh_bind_group, &[]);
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
        let pipeline = pipeline_manager.get_render_pipeline(self.render_pipe_render_atmosphere)?;

        rpass.push_debug_group("Raymarch Atmosphere");
        rpass.set_bind_group(1, &self.raymarch_bindgroup, &[]);
        rpass.set_pipeline(pipeline);
        rpass.draw(0..3, 0..1);
        rpass.pop_debug_group();

        Ok(())
    }
}

fn generate_sampling_directions(num_samples: u32) -> Vec<Vec3RowPadded> {
    use crate::sampling::halton;

    (0..num_samples)
        .map(|i| {
            let z = halton(i, 2) * 2.0 - 1.0;
            let t = halton(i, 3) * std::f32::consts::TAU;
            let r = (1.0 - z * z).sqrt();

            let x = r * t.cos();
            let y = r * t.sin();

            glam::vec3(x, y, z).into()
        })
        .collect()
}
