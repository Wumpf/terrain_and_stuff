#[cfg(not(target_arch = "wasm32"))]
mod main_desktop;
#[cfg(target_arch = "wasm32")]
mod main_web;
#[cfg(target_arch = "wasm32")]
mod shaders_embedded;

mod atmosphere;
mod camera;
mod primary_depth_buffer;
mod render_output;
mod resource_managers;
mod result_ext;
mod terrain;
mod wgpu_error_handling;
mod wgpu_utils;

// -----------------------------------------

use anyhow::Context;
use minifb::{Window, WindowOptions};
use std::sync::{atomic::AtomicU64, Arc};
use web_time::Instant;

use atmosphere::Atmosphere;
use camera::Camera;
use primary_depth_buffer::PrimaryDepthBuffer;
use render_output::{HdrBackbuffer, Screen};
use resource_managers::{GlobalBindings, PipelineManager};
use result_ext::ResultExt;
use terrain::TerrainRenderer;
use wgpu_error_handling::{ErrorTracker, WgpuErrorScope};

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;

struct Application<'a> {
    screen: Screen<'a>,
    global_bindings: GlobalBindings,
    hdr_backbuffer: HdrBackbuffer,
    primary_depth_buffer: PrimaryDepthBuffer,

    atmosphere: Atmosphere,
    terrain: TerrainRenderer,

    window: Window,
    adapter: wgpu::Adapter,
    device: Arc<wgpu::Device>,
    queue: wgpu::Queue,
    camera: Camera,
    last_update: Instant,

    active_frame_index: u64,
    frame_index_for_uncaptured_errors: Arc<AtomicU64>,
    pipeline_manager: PipelineManager,
    error_tracker: Arc<ErrorTracker>,
}

impl<'a> Application<'a> {
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

        let window = Window::new(
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

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Device"),
                    required_features: wgpu::FeaturesWebGPU::DUAL_SOURCE_BLENDING.into(),
                    // Useful for debugging.
                    //#[cfg(not(target_arch = "wasm32"))]
                    //required_features: wgpu::FeaturesWebGPU::POLYGON_MODE_LINE,
                    ..Default::default()
                },
                None,
            )
            .await
            .context("Failed to create device")?;

        // Make all errors forward to the console before panicking, this way they also show up on the web!
        let error_tracker = Arc::new(ErrorTracker::default());

        let mut pipeline_manager = PipelineManager::new().context("Create pipeline manager")?;

        let resolution = glam::uvec2(window.get_size().0 as _, window.get_size().1 as _);
        let screen = Screen::new(&device, &adapter, surface, resolution);
        let primary_depth_buffer = PrimaryDepthBuffer::new(&device, resolution);
        let global_bindings = GlobalBindings::new(&device);
        let hdr_backbuffer = HdrBackbuffer::new(
            &device,
            resolution,
            &mut pipeline_manager,
            screen.surface_format(),
        )
        .context("Create HDR backbuffer & display transform pipeline")?;

        let atmosphere = Atmosphere::new(
            &device,
            &global_bindings,
            &mut pipeline_manager,
            &primary_depth_buffer,
        )
        .context("Create sky renderer")?;
        let terrain =
            TerrainRenderer::new(&device, &queue, &global_bindings, &mut pipeline_manager)
                .context("Create terrain renderer")?;

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

        Ok(Application {
            screen,
            global_bindings,
            hdr_backbuffer,
            primary_depth_buffer,
            atmosphere,
            terrain,
            window,

            adapter,
            device: Arc::new(device),
            queue,
            camera: Camera::new(),
            last_update: Instant::now(),

            active_frame_index: 0,
            frame_index_for_uncaptured_errors,
            pipeline_manager,
            error_tracker,
        })
    }

    pub fn update(&mut self) {
        let current_time = Instant::now();
        let delta_time = current_time.duration_since(self.last_update).as_secs_f32();
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

        self.camera.update(delta_time, &self.window);
    }

    pub fn draw(&mut self) {
        let error_scope = WgpuErrorScope::start(&self.device);

        let Some(frame) = self.screen.start_frame(&self.device) else {
            return;
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let aspect_ratio = self.screen.aspect_ratio();
        let view_from_world = self.camera.view_from_world();
        let projection_from_view = self.camera.projection_from_view(aspect_ratio);
        self.global_bindings.update_frame_uniform_buffer(
            &self.queue,
            &resource_managers::FrameUniformBuffer {
                view_from_world: view_from_world.into(),
                projection_from_view: projection_from_view.into(),
                projection_from_world: (projection_from_view * view_from_world).into(),
                camera_position: self.camera.position().into(),
                camera_forward: self.camera.forward().into(),
                tan_half_fov: self.camera.tan_half_fov(aspect_ratio).into(),
            },
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main encoder"),
            });

        self.draw_scene(&mut encoder);
        self.hdr_backbuffer
            .display_transform(&view, &mut encoder, &self.pipeline_manager)
            .ok_or_log("display transform");

        let command_buffer = encoder.finish();
        self.queue.submit(Some(command_buffer));
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
    }

    fn draw_scene(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.atmosphere
            .prepare(encoder, &self.pipeline_manager)
            .ok_or_log("prepare sky");

        {
            let mut hdr_rpass_with_depth = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Primary HDR render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.hdr_backbuffer.texture_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.primary_depth_buffer.view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // Near plane is at 0, infinity is at 1.
                        // Need to store depth for sky raymarching.
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            hdr_rpass_with_depth.set_bind_group(0, &self.global_bindings.bind_group, &[]);

            self.terrain
                .draw(&mut hdr_rpass_with_depth, &self.pipeline_manager)
                .ok_or_log("draw sky");
        }
        {
            let mut hdr_rpass_without_depth =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Atmosphere HDR render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: self.hdr_backbuffer.texture_view(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

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
