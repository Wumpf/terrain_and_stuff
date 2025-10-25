#[cfg(not(target_arch = "wasm32"))]
mod main_desktop;
#[cfg(target_arch = "wasm32")]
mod main_web;
#[cfg(target_arch = "wasm32")]
mod shaders_embedded;

mod atmosphere;
mod bluenoise;
mod camera;
mod config;
mod egui_minifb;
mod gui;
mod primary_depth_buffer;
mod render_output;
mod resource_managers;
mod result_ext;
mod shadowmap;
mod terrain;
mod wgpu_error_handling;
mod wgpu_utils;

// -----------------------------------------

pub type EncoderScope<'a> = wgpu_profiler::Scope<'a, wgpu::CommandEncoder>;

// -----------------------------------------

use anyhow::Context;
use egui_minifb::EguiMinifb;
use minifb::{Window, WindowOptions};
use std::sync::{Arc, atomic::AtomicU64};
use web_time::Instant;

use atmosphere::Atmosphere;
use primary_depth_buffer::PrimaryDepthBuffer;
use render_output::{HdrBackbuffer, Screen};
use resource_managers::{GlobalBindings, PipelineManager};
use result_ext::ResultExt;
use terrain::TerrainRenderer;
use wgpu_error_handling::{ErrorTracker, WgpuErrorScope};

use crate::{bluenoise::BluenoiseTextures, config::Config, shadowmap::Shadowmap};

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;

const CONFIG_FILE_PATH: &str = "config.ron";

struct Application<'a> {
    screen: Screen<'a>,
    global_bindings: GlobalBindings,
    hdr_backbuffer: HdrBackbuffer,
    primary_depth_buffer: PrimaryDepthBuffer,

    gpu_profiler: Option<wgpu_profiler::GpuProfiler>,

    gui: EguiMinifb,

    atmosphere: Atmosphere,
    terrain: TerrainRenderer,
    shadowmap: Shadowmap,

    window: Window,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    last_update: Instant,

    active_frame_index: u64,
    frame_index_for_uncaptured_errors: Arc<AtomicU64>,
    pipeline_manager: PipelineManager,
    error_tracker: Arc<ErrorTracker>,

    last_gpu_profiler_results: Vec<Vec<wgpu_profiler::GpuTimerQueryResult>>,

    config: Config,
}

const NUM_PROFILER_RESULTS_TO_KEEP: usize = 10;

impl Application<'_> {
    /// Initializes the application.
    ///
    /// There's various ways for this to fail, all of which are handled via `expect` right now.
    /// Of course there's be better ways to handle these (e.g. show something nice on screen or try a bit harder).
    async fn new() -> anyhow::Result<Self> {
        let instance = wgpu::util::new_instance_with_webgpu_detection(&wgpu::InstanceDescriptor {
            // Kick out DX12 & GL to limit variation - GL isn't terribly stable and feature complete, DX12 is always pain with shader compilation.
            backends: wgpu::Backends::VULKAN
                | wgpu::Backends::METAL
                | wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        })
        .await;

        let mut window = Window::new(
            "terrain_and_stuff",
            WIDTH,
            HEIGHT,
            WindowOptions {
                resize: true,
                ..Default::default()
            },
        )?;

        // Unfortunately, mini_fb's window type isn't `Send` which is required for wgpu's `WindowHandle` trait.
        // We instead have to use the unsafe variant to create a surface directly from the window handle.
        //
        // SAFETY:
        // * The window handles are valid at this point
        // * The window is guaranteed to outlive the surface since we're ensuring so in `Application's` Drop impl
        let surface = unsafe {
            instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::from_window(&window)
                    .expect("Failed to create surface target."),
            )
        }
        .context("Failed to create surface")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .context("Failed to find an appropriate adapter")?;
        log::info!("Created wgpu adapter: {:?}", adapter.get_info());

        let optional_features = wgpu_profiler::GpuProfiler::ALL_WGPU_TIMER_FEATURES;
        let required_features = wgpu::FeaturesWebGPU::DUAL_SOURCE_BLENDING;
        let required_limits = wgpu::Limits {
            // Using larger workgroups makes sky SH convolution shader simpler.
            // 1024 is widely supported
            // https://web3dsurvey.com/webgpu/limits/maxComputeWorkgroupSizeX
            // https://web3dsurvey.com/webgpu/limits/maxComputeInvocationsPerWorkgroup
            max_compute_workgroup_size_x: 1024,
            max_compute_invocations_per_workgroup: 1024,

            ..wgpu::Limits::default()
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Device"),
                required_features: wgpu::Features::from(required_features)
                    | optional_features.intersection(adapter.features()),
                // Useful for debugging.
                //#[cfg(not(target_arch = "wasm32"))]
                //required_features: required_features | wgpu::FeaturesWebGPU::POLYGON_MODE_LINE,
                required_limits,
                memory_hints: wgpu::MemoryHints::Performance,
                ..Default::default()
            })
            .await
            .context("Failed to create device")?;

        // Make all errors forward to the console before panicking, this way they also show up on the web!
        let error_tracker = Arc::new(ErrorTracker::default());

        let mut pipeline_manager = PipelineManager::new().context("Create pipeline manager")?;

        let resolution = glam::uvec2(window.get_size().0 as _, window.get_size().1 as _);
        let screen = Screen::new(&device, &adapter, surface, resolution);
        let primary_depth_buffer = PrimaryDepthBuffer::new(&device, resolution);
        let bluenoise = BluenoiseTextures::load(&device, &queue);
        let global_bindings = GlobalBindings::new(&device, &bluenoise);
        let hdr_backbuffer = HdrBackbuffer::new(
            &device,
            &queue,
            &global_bindings,
            resolution,
            &mut pipeline_manager,
            screen.surface_format(),
        )
        .context("Create HDR backbuffer & display transform pipeline")?;

        let gui = EguiMinifb::new(&device, &screen);
        window.set_input_callback(Box::new(gui.text_callback()));

        let atmosphere = Atmosphere::new(
            &device,
            &global_bindings,
            &mut pipeline_manager,
            &primary_depth_buffer,
        )
        .context("Create sky renderer")?;
        let terrain = TerrainRenderer::new(
            &device,
            &queue,
            &global_bindings,
            atmosphere.sun_and_sky_lighting_params_buffer(),
            &mut pipeline_manager,
        )
        .context("Create terrain renderer")?;
        let shadowmap = Shadowmap::new(&device);

        // Now that initialization is over (!), make sure to catch all errors, never crash, and deduplicate reported errors.
        // `on_uncaptured_error` is a last-resort handler which we should never hit,
        // since there should always be an open error scope.
        //
        // Note that this handler may not be called for all errors!
        // (as of writing, wgpu-core will always call it when there's no open error scope, but Dawn doesn't!)
        // Therefore, it is important to always have a `WgpuErrorScope` open!
        // See https://www.w3.org/TR/webgpu/#telemetry
        let frame_index_for_uncaptured_errors = Arc::new(AtomicU64::new(0));
        device.on_uncaptured_error({
            let error_tracker = Arc::clone(&error_tracker);
            let frame_index_for_uncaptured_errors = frame_index_for_uncaptured_errors.clone();
            Box::new(move |err| {
                error_tracker.handle_error(
                    err,
                    frame_index_for_uncaptured_errors.load(std::sync::atomic::Ordering::Acquire),
                );
            })
        });

        let gpu_profiler = wgpu_profiler::GpuProfiler::new(
            &device,
            wgpu_profiler::GpuProfilerSettings {
                enable_timer_queries: true,
                enable_debug_groups: true,
                max_num_pending_frames: 2,
            },
        )
        .unwrap();

        let config = Config::load_from_ron_file_or_default_and_log_error(CONFIG_FILE_PATH);

        Ok(Application {
            screen,
            global_bindings,
            hdr_backbuffer,
            primary_depth_buffer,

            gpu_profiler: Some(gpu_profiler),

            gui,

            atmosphere,
            terrain,
            shadowmap,

            window,

            adapter,
            device,
            queue,
            last_update: Instant::now(),

            active_frame_index: 0,
            frame_index_for_uncaptured_errors,
            pipeline_manager,
            error_tracker,

            last_gpu_profiler_results: Vec::new(),

            config,
        })
    }

    pub fn update(&mut self) {
        let current_time = Instant::now();
        let delta_time = current_time.duration_since(self.last_update);

        // Very crude FPS limiter:
        let delta_time = if let Some(target_fps) = self.config.target_fps {
            let target_delta_time = std::time::Duration::from_secs_f32(1.0 / target_fps as f32);
            if let Some(sleep_time) = target_delta_time.checked_sub(delta_time) {
                std::thread::sleep(sleep_time);
            }
            target_delta_time
        } else {
            delta_time
        };

        self.last_update = current_time;

        self.active_frame_index += 1;
        self.pipeline_manager.reload_changed_pipelines(&self.device);

        let current_resolution =
            glam::uvec2(self.window.get_size().0 as _, self.window.get_size().1 as _);

        if self.screen.resolution() != current_resolution
            // Ignore zero sized windows, lots of resize operations can't handle this.
            && current_resolution.x != 0
            && current_resolution.y != 0
        {
            self.screen.on_resize(&self.device, current_resolution);
            self.hdr_backbuffer
                .on_resize(&self.device, current_resolution);
            self.primary_depth_buffer = PrimaryDepthBuffer::new(&self.device, current_resolution);
            self.atmosphere
                .on_resize(&self.device, &self.primary_depth_buffer);
        }

        while let Some(new_profiler_results) = self
            .gpu_profiler
            .as_mut()
            .unwrap()
            .process_finished_frame(self.queue.get_timestamp_period())
        {
            if self.last_gpu_profiler_results.len() + 1 >= NUM_PROFILER_RESULTS_TO_KEEP {
                self.last_gpu_profiler_results.remove(0);
            }
            self.last_gpu_profiler_results.push(new_profiler_results);
        }

        let config_before = self.config.clone();

        let mut mouse_does_ui_interaction = false;
        self.gui.update(&self.window, |egui_ctx| {
            gui::run_gui(
                egui_ctx,
                &self.last_gpu_profiler_results,
                &mut mouse_does_ui_interaction,
                &mut self.config,
            );
        });

        if !mouse_does_ui_interaction {
            self.config.camera.update(delta_time, &self.window);
        }

        // Save if config changed and mouse & keyboard keys aren't down right now (which is associated with camera movement)
        if config_before != self.config
            && !self.window.get_mouse_down(minifb::MouseButton::Left)
            && self.window.get_keys().is_empty()
        {
            self.config.save_to_ron_file_or_log_error(CONFIG_FILE_PATH);
        }
    }

    pub fn draw(&mut self) {
        let error_scope = WgpuErrorScope::start(&self.device);

        let Some(frame) = self.screen.start_frame(&self.device) else {
            return;
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let camera = &self.config.camera;

        let aspect_ratio = self.screen.aspect_ratio();
        let view_from_world = camera.view_from_world();
        let projection_from_view = camera.projection_from_view(aspect_ratio);
        let shadow_map_from_world = self.shadowmap.shadow_projection_from_world(
            self.config.sun_angles.dir_to_sun(),
            self.terrain.bounding_box(),
        );

        self.global_bindings.update_frame_uniform_buffer(
            &self.queue,
            &resource_managers::FrameUniformBuffer {
                view_from_world: view_from_world.into(),
                projection_from_view: projection_from_view.into(),
                projection_from_world: (projection_from_view * view_from_world).into(),
                shadow_map_from_world: shadow_map_from_world.into(),
                camera_position: camera.position.into(),
                camera_forward: camera.forward().into(),
                tan_half_fov: camera.tan_half_fov(aspect_ratio).into(),
                dir_to_sun: self.config.sun_angles.dir_to_sun().into(),
            },
        );

        let mut gpu_profiler = self.gpu_profiler.take().unwrap();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main encoder"),
            });

        {
            let mut encoder = gpu_profiler.scope("root", &mut encoder);

            self.draw_scene(&mut encoder);
            {
                let pass_query = gpu_profiler
                    .begin_pass_query("Display transform & GUI", &mut encoder)
                    .with_parent(encoder.scope.as_ref());
                let timestamp_writes = pass_query.render_pass_timestamp_writes();

                let mut render_pass = wgpu_profiler::OwningScope {
                    profiler: &gpu_profiler,
                    recorder: encoder
                        .begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Display transform & GUI"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes,
                            occlusion_query_set: None,
                        })
                        .forget_lifetime(),
                    scope: Some(pass_query),
                };

                self.hdr_backbuffer
                    .display_transform(
                        &mut render_pass,
                        &self.pipeline_manager,
                        &self.global_bindings,
                    )
                    .ok_or_log("display transform");
                self.gui
                    .draw(&self.device, &self.queue, &mut encoder, &mut render_pass);
            }
        }

        gpu_profiler.resolve_queries(&mut encoder);

        let command_buffer = encoder.finish();
        self.queue.submit(Some(command_buffer));

        gpu_profiler.end_frame().unwrap();

        frame.present();

        {
            let frame_index_for_uncaptured_errors = self.frame_index_for_uncaptured_errors.clone();
            self.error_tracker.handle_error_future(
                self.adapter.get_info().backend,
                error_scope.end(),
                self.active_frame_index,
                move |err_tracker, frame_index| {
                    // Update last completed frame index.
                    //
                    // Note that this means that the device timeline has now finished this frame as well!
                    // Reminder: On WebGPU the device timeline may be arbitrarily behind the content timeline!
                    // See <https://www.w3.org/TR/webgpu/#programming-model-timelines>.
                    frame_index_for_uncaptured_errors
                        .store(frame_index, std::sync::atomic::Ordering::Release);
                    err_tracker.on_device_timeline_frame_finished(frame_index);
                },
            );
        }

        self.gpu_profiler = Some(gpu_profiler);
    }

    fn draw_scene(&self, encoder: &mut EncoderScope<'_>) {
        self.atmosphere
            .prepare(
                &self.queue,
                encoder,
                &self.pipeline_manager,
                &self.global_bindings,
                &self.config.atmosphere_params,
            )
            .ok_or_log("prepare sky");

        {
            let mut shadowmap_rpass = encoder.scoped_render_pass(
                "Shadowmap render pass",
                self.shadowmap.shadow_map_render_pass_descriptor(),
            );
            self.terrain
                .draw_shadowmap(&mut shadowmap_rpass, &self.pipeline_manager)
                .ok_or_log("draw shadowmap");
        }
        {
            let mut hdr_rpass_with_depth = encoder.scoped_render_pass(
                "Primary HDR render pass",
                wgpu::RenderPassDescriptor {
                    label: Some("Primary HDR"),
                    color_attachments: &[Some(self.hdr_backbuffer.color_attachment())],
                    depth_stencil_attachment: Some(
                        self.primary_depth_buffer.depth_stencil_attachment(),
                    ),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                },
            );

            hdr_rpass_with_depth.set_bind_group(0, &self.global_bindings.bind_group, &[]);

            self.terrain
                .draw(&mut hdr_rpass_with_depth, &self.pipeline_manager)
                .ok_or_log("draw sky");
        }
        {
            let mut hdr_rpass_without_depth = encoder.scoped_render_pass(
                "Atmosphere HDR render pass",
                wgpu::RenderPassDescriptor {
                    label: Some("Atmosphere HDR"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: self.hdr_backbuffer.texture_view(),
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                },
            );

            hdr_rpass_without_depth.set_bind_group(0, &self.global_bindings.bind_group, &[]);

            self.atmosphere
                .draw(&mut hdr_rpass_without_depth, &self.pipeline_manager)
                .ok_or_log("draw sky");
        }
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    return; // Not used on web, this method is merely a placeholder.

    #[cfg(not(target_arch = "wasm32"))]
    main_desktop::main_desktop().unwrap();
}
