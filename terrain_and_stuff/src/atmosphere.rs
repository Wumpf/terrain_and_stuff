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
        wgpu_buffer_types::{BoolAsInteger, Vec3RowPadded, WgslEnum},
    },
};

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    bytemuck::Zeroable,
    bytemuck::CheckedBitPattern,
    bytemuck::Contiguous,
    serde::Serialize,
    serde::Deserialize,
)]
#[repr(u32)]
pub enum AtmosphereDebugDrawMode {
    None = 0,
    Sh = 1,
    NoGeometryOverlay = 2,
    TransmittanceLut = 3,
    MultipleScatteringLut = 4,
}

impl std::fmt::Display for AtmosphereDebugDrawMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtmosphereDebugDrawMode::None => write!(f, "None"),
            AtmosphereDebugDrawMode::Sh => write!(f, "Spherical harmonics"),
            AtmosphereDebugDrawMode::NoGeometryOverlay => write!(f, "No geometry overlay"),
            AtmosphereDebugDrawMode::TransmittanceLut => write!(f, "Transmittance LUT"),
            AtmosphereDebugDrawMode::MultipleScatteringLut => write!(f, "Multiple scattering LUT"),
        }
    }
}

impl From<AtmosphereDebugDrawMode> for u32 {
    fn from(value: AtmosphereDebugDrawMode) -> Self {
        value as u32
    }
}

/// Parameters for the atmosphere other than light direction.
///
/// This is GPU uploaded every frame for simplicity.
/// (small'ish structs like this don't make a dent perf wise 🤷)
#[derive(
    Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod, serde::Serialize, serde::Deserialize,
)]
#[repr(C)]
pub struct AtmosphereParams {
    pub draw_mode: WgslEnum<AtmosphereDebugDrawMode>,

    // Atmosphere values for earth.
    pub ground_radius_km: f32,
    pub atmosphere_radius_km: f32,

    pub rayleigh_scale_height: f32,
    // -- row boundary --
    pub rayleigh_scattering_per_km_density: glam::Vec3,

    pub mie_scale_height: f32,
    // -- row boundary --
    pub mie_scattering_per_km_density: f32,
    pub mie_absorption_per_km_density: f32,

    // Sun's angle is 0.5 degrees according to this.
    // https://www.nasa.gov/wp-content/uploads/2015/01/YOSS_Act_9.pdf
    //const sun_diameter_rad = 0.5 * DEG_TO_RAD;
    // But it doesn't look that nice:
    // we'd need some really heavy bloom to account for the fact that this is an excruciatingly bright spot.
    // See also `sun_unscattered_luminance` below.
    pub sun_disk_diameter_rad: f32,

    // When directly looking at the sun.. _waves hands_.. the maths breaks down and we just want to draw a white spot, okay? ;-)
    pub sun_disk_illuminance_factor: f32,
    // -- row boundary --
    pub ozone_absorption_per_km_density: glam::Vec3,
    pub enable_multiple_scattering: BoolAsInteger,
    // -- row boundary --
    pub sun_illuminance: Vec3RowPadded,
    // -- row boundary --
    pub ground_albedo: Vec3RowPadded,
    // -- row boundary --
}

impl Default for AtmosphereParams {
    fn default() -> Self {
        Self {
            draw_mode: WgslEnum::<AtmosphereDebugDrawMode>::new(AtmosphereDebugDrawMode::None),

            // Atmosphere values for earth.
            ground_radius_km: 6360.0,
            atmosphere_radius_km: 6460.0,

            rayleigh_scale_height: 8.0,
            rayleigh_scattering_per_km_density: glam::vec3(0.005802, 0.013558, 0.033100),

            mie_scale_height: 1.2,
            mie_scattering_per_km_density: 0.003996,
            mie_absorption_per_km_density: 0.004440,

            ozone_absorption_per_km_density: glam::vec3(0.000650, 0.001881, 0.000085),
            enable_multiple_scattering: true.into(),

            // Roughly the intensity Sun without any scattering
            // https://en.wikipedia.org/wiki/Luminance
            //const sun_unscattered_luminance: vec3f = vec3f(1.6, 1.6, 1.6) * 1000000000.0;
            // Okay, that's just too much to work with practically 🤷
            // Instead we just use the sun as the grounding measure of things.
            sun_illuminance: glam::vec3(1.6, 1.6, 1.6).into(),

            // Sun's angle is 0.5 degrees according to this.
            // https://www.nasa.gov/wp-content/uploads/2015/01/YOSS_Act_9.pdf
            //const sun_diameteter_rad = 0.5 * DEG_TO_RAD;
            // But it doesn't look that nice:
            // we'd need some really heavy bloom to account for the fact that this is an excrucingly bright spot.
            // See also `sun_unscattered_luminance` below.
            sun_disk_diameter_rad: 1.0 * std::f32::consts::TAU / 360.0,

            // When directly looking at the sun... _waves hands_... the maths breaks down, and we just want to draw a white spot, okay? ;-)
            sun_disk_illuminance_factor: 100.0,

            ground_albedo: glam::vec3(0.3, 0.3, 0.3).into(),
        }
    }
}

/// More like sun direction I guess.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SunAngles {
    /// Sun's azimuth angle in radians.
    pub sun_azimuth: f32,
    /// Sun's altitude angle in radians.
    pub sun_altitude: f32,
}

impl Default for SunAngles {
    fn default() -> Self {
        Self {
            sun_azimuth: std::f32::consts::PI,
            sun_altitude: std::f32::consts::PI / 4.0,
        }
    }
}

impl SunAngles {
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
    render_pipe_lut_transmittance: RenderPipelineHandle,
    render_pipe_lut_multiple_scattering: RenderPipelineHandle,
    render_pipe_render_atmosphere: RenderPipelineHandle,

    compute_pipe_sh: ComputePipelineHandle,

    atmosphere_params_bindgroup: wgpu::BindGroup,
    render_lut_multiple_scattering_bindgroup: wgpu::BindGroup,
    compute_sh_bind_group: wgpu::BindGroup,

    render_atmosphere_bindgroup_main: wgpu::BindGroup,
    render_atmosphere_bindings_screen_dependent: BindGroupLayoutWithDesc,
    render_atmosphere_bindgroup_screen_dependent: wgpu::BindGroup,

    lut_transmittance: wgpu::TextureView,
    lut_multiple_scattering: wgpu::TextureView,
    atmosphere_params_buffer: wgpu::Buffer,
    sky_and_sun_lighting_params_buffer: wgpu::Buffer,
}

const LUT_TRANSMITTANCE_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 256,
    height: 64,
    depth_or_array_layers: 1,
};

const LUT_MULTIPLE_SCATTERING_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 64,
    height: 64,
    depth_or_array_layers: 1,
};

impl Atmosphere {
    pub fn new(
        device: &wgpu::Device,
        global_bindings: &GlobalBindings,
        pipeline_manager: &mut PipelineManager,
        primary_depth_buffer: &PrimaryDepthBuffer,
    ) -> Result<Self, PipelineError> {
        let sh_coefficients_buffer_size = (1 + 3 + 5 // SH bands 0, 1, 2
            + 1) * // Sun illuminance.
            (size_of::<Vec3RowPadded>() as u64); // RGB for each band, need to add padding
        let sky_and_sun_lighting_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SH coefficients"),
            size: sh_coefficients_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let atmosphere_params = AtmosphereParams::default();
        let atmosphere_params_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Atmosphere params"),
                contents: bytemuck::bytes_of(&atmosphere_params),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let atmosphere_params_bindings = BindGroupLayoutBuilder::new()
            .next_binding(
                wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(atmosphere_params_buffer.size()),
                },
            )
            .create(device, "atmosphere_params");
        let atmosphere_params_bindgroup = BindGroupBuilder::new(&atmosphere_params_bindings)
            .buffer(atmosphere_params_buffer.as_entire_buffer_binding())
            .create(device, "atmosphere_params");

        // Transmittance.
        let (lut_transmittance, render_pipe_lut_transmittance) = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("lut_transmittance"),
                bind_group_layouts: &[
                    &global_bindings.bind_group_layout.layout,
                    &atmosphere_params_bindings.layout,
                ],
                push_constant_ranges: &[],
            });

            let lut_transmittance = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Transmittance LUT"),
                size: LUT_TRANSMITTANCE_SIZE,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let lut_transmittance =
                lut_transmittance.create_view(&wgpu::TextureViewDescriptor::default());

            let render_pipe_lut_transmittance = pipeline_manager.create_render_pipeline(
                device,
                RenderPipelineDescriptor {
                    debug_label: "lut_transmittance".to_owned(),
                    layout,
                    vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                    fragment_shader: Some(ShaderEntryPoint::first_in(
                        "atmosphere/lut_transmittance.wgsl",
                    )),
                    fragment_targets: vec![HdrBackbuffer::FORMAT.into()],
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                },
            )?;

            (lut_transmittance, render_pipe_lut_transmittance)
        };

        // Multiple scattering.
        let (
            lut_multiple_scattering,
            render_lut_multiple_scattering_bindgroup,
            render_pipe_lut_multiple_scattering,
        ) = {
            // Need the transmittance lut to compute multiple scattering lut.
            let lut_multiple_scattering_bindings = BindGroupLayoutBuilder::new()
                .next_binding_fragment(wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                })
                .create(device, "lut_multiple_scattering");
            let render_lut_multiple_scattering_bindgroup =
                BindGroupBuilder::new(&lut_multiple_scattering_bindings)
                    .texture(&lut_transmittance)
                    .create(device, "lut_multiple_scattering");

            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("lut_multiple_scattering"),
                bind_group_layouts: &[
                    &global_bindings.bind_group_layout.layout,
                    &atmosphere_params_bindings.layout,
                    &lut_multiple_scattering_bindings.layout,
                ],
                push_constant_ranges: &[],
            });

            let lut_multiple_scattering = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Transmittance LUT"),
                size: LUT_MULTIPLE_SCATTERING_SIZE,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let lut_multiple_scattering =
                lut_multiple_scattering.create_view(&wgpu::TextureViewDescriptor::default());

            let render_pipe_lut_multiple_scattering = pipeline_manager.create_render_pipeline(
                device,
                RenderPipelineDescriptor {
                    debug_label: "lut_multiple_scattering".to_owned(),
                    layout,
                    vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                    fragment_shader: Some(ShaderEntryPoint::first_in(
                        "atmosphere/lut_multiple_scattering.wgsl",
                    )),
                    fragment_targets: vec![HdrBackbuffer::FORMAT.into()],
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                },
            )?;

            (
                lut_multiple_scattering,
                render_lut_multiple_scattering_bindgroup,
                render_pipe_lut_multiple_scattering,
            )
        };

        let render_atmosphere_bindings_main = BindGroupLayoutBuilder::new()
            // [in] transmittance lut
            .next_binding_fragment(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
            // [in] multiple scattering lut
            .next_binding_fragment(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
            // [in] sun color + sh coefficients
            .next_binding_fragment(wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(sh_coefficients_buffer_size),
            })
            .create(device, "render_atmosphere_main");

        let render_atmosphere_bindgroup_main =
            BindGroupBuilder::new(&render_atmosphere_bindings_main)
                .texture(&lut_transmittance)
                .texture(&lut_multiple_scattering)
                .buffer(sky_and_sun_lighting_params_buffer.as_entire_buffer_binding())
                .create(device, "render_atmosphere_main");

        let render_atmosphere_bindings_screen_dependent = BindGroupLayoutBuilder::new()
            // [in] depth buffer
            .next_binding_fragment(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            })
            .create(device, "render_atmosphere_screen_dependent");

        // Render atmosphere.
        let render_pipe_render_atmosphere = {
            let raymarch_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render_atmosphere"),
                bind_group_layouts: &[
                    &global_bindings.bind_group_layout.layout,
                    &atmosphere_params_bindings.layout,
                    &render_atmosphere_bindings_main.layout,
                    &render_atmosphere_bindings_screen_dependent.layout,
                ],
                push_constant_ranges: &[],
            });
            pipeline_manager.create_render_pipeline(
                device,
                RenderPipelineDescriptor {
                    debug_label: "render_atmosphere".to_owned(),
                    layout: raymarch_layout,
                    vertex_shader: ShaderEntryPoint::first_in("screen_triangle.wgsl"),
                    fragment_shader: Some(ShaderEntryPoint::first_in(
                        "atmosphere/render_atmosphere.wgsl",
                    )),
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
        let render_atmosphere_bindgroup_screen_dependent =
            Self::create_render_atmosphere_bindgroup_screen_dependent(
                device,
                &render_atmosphere_bindings_screen_dependent,
                primary_depth_buffer,
            );

        // Compute pipeline for computing SH coefficients.
        let (compute_pipe_sh, compute_sh_bind_group) = {
            let bindings = BindGroupLayoutBuilder::new()
                // [in] transmittance lut
                .next_binding_compute(wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                })
                // [in] multiple scattering lut
                .next_binding_compute(wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                })
                // [out] sun color + sh coefficients
                .next_binding_compute(wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(sh_coefficients_buffer_size),
                })
                .create(device, "atmosphere/sh");

            let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compute sh layout"),
                bind_group_layouts: &[
                    &global_bindings.bind_group_layout.layout,
                    &atmosphere_params_bindings.layout,
                    &bindings.layout,
                ],
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
                .texture(&lut_transmittance)
                .texture(&lut_multiple_scattering)
                .buffer(sky_and_sun_lighting_params_buffer.as_entire_buffer_binding())
                .create(device, "atmosphere/compute_sh");

            (pipeline, compute_sh_bind_group)
        };

        Ok(Self {
            render_pipe_lut_transmittance,
            render_pipe_lut_multiple_scattering,
            render_pipe_render_atmosphere,
            compute_pipe_sh,

            atmosphere_params_bindgroup,
            render_lut_multiple_scattering_bindgroup,
            compute_sh_bind_group,

            render_atmosphere_bindgroup_main,
            render_atmosphere_bindings_screen_dependent,
            render_atmosphere_bindgroup_screen_dependent,

            sky_and_sun_lighting_params_buffer,
            lut_transmittance,
            lut_multiple_scattering,
            atmosphere_params_buffer,
        })
    }

    fn create_render_atmosphere_bindgroup_screen_dependent(
        device: &wgpu::Device,
        render_atmosphere_bindings_screen_dependent: &BindGroupLayoutWithDesc,
        primary_depth_buffer: &PrimaryDepthBuffer,
    ) -> wgpu::BindGroup {
        BindGroupBuilder::new(render_atmosphere_bindings_screen_dependent)
            .texture(primary_depth_buffer.view())
            .create(device, "render_atmosphere_screen_dependent")
    }

    pub fn on_resize(&mut self, device: &wgpu::Device, primary_depth_buffer: &PrimaryDepthBuffer) {
        self.render_atmosphere_bindgroup_screen_dependent =
            Self::create_render_atmosphere_bindgroup_screen_dependent(
                device,
                &self.render_atmosphere_bindings_screen_dependent,
                primary_depth_buffer,
            );
    }

    pub fn sun_and_sky_lighting_params_buffer(&self) -> &wgpu::Buffer {
        &self.sky_and_sun_lighting_params_buffer
    }

    pub fn prepare(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut EncoderScope<'_>,
        pipeline_manager: &PipelineManager,
        global_bindings: &GlobalBindings,
        parameters: &AtmosphereParams,
    ) -> Result<(), PipelineError> {
        queue.write_buffer(
            &self.atmosphere_params_buffer,
            0,
            bytemuck::bytes_of(parameters),
        );

        let mut encoder = encoder.scope("Prepare atmosphere");
        {
            let mut compute_pass = encoder.scoped_compute_pass("SH coefficients");
            compute_pass.set_pipeline(pipeline_manager.get_compute_pipeline(self.compute_pipe_sh)?);
            compute_pass.set_bind_group(0, &global_bindings.bind_group, &[]);
            compute_pass.set_bind_group(1, &self.atmosphere_params_bindgroup, &[]);
            compute_pass.set_bind_group(2, &self.compute_sh_bind_group, &[]);
            compute_pass.dispatch_workgroups(1, 1, 1);
        }

        // TODO: compute luts only if parameters have changed.
        {
            let mut render_pass = encoder.scoped_render_pass(
                "Transmittance LUT",
                wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.lut_transmittance,
                        depth_slice: None,
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

            render_pass.set_bind_group(0, &global_bindings.bind_group, &[]);
            render_pass.set_bind_group(1, &self.atmosphere_params_bindgroup, &[]);
            render_pass.set_pipeline(
                pipeline_manager.get_render_pipeline(self.render_pipe_lut_transmittance)?,
            );
            render_pass.draw(0..3, 0..1);
        }

        if parameters.enable_multiple_scattering.into() {
            let mut render_pass = encoder.scoped_render_pass(
                "Multiple scattering LUT",
                wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.lut_multiple_scattering,
                        depth_slice: None,
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

            render_pass.set_bind_group(0, &global_bindings.bind_group, &[]);
            render_pass.set_bind_group(1, &self.atmosphere_params_bindgroup, &[]);
            render_pass.set_bind_group(2, &self.render_lut_multiple_scattering_bindgroup, &[]);
            render_pass.set_pipeline(
                pipeline_manager.get_render_pipeline(self.render_pipe_lut_multiple_scattering)?,
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
        rpass.set_bind_group(1, &self.atmosphere_params_bindgroup, &[]);
        rpass.set_bind_group(2, &self.render_atmosphere_bindgroup_main, &[]);
        rpass.set_bind_group(3, &self.render_atmosphere_bindgroup_screen_dependent, &[]);
        rpass.set_pipeline(pipeline);
        rpass.draw(0..3, 0..1);
        rpass.pop_debug_group();

        Ok(())
    }
}
