#[cfg(not(target_arch = "wasm32"))]
mod main_desktop;
#[cfg(target_arch = "wasm32")]
mod main_web;
#[cfg(target_arch = "wasm32")]
mod shaders_embedded;

mod resource_managers;
mod wgpu_error_handling;

// -----------------------------------------

use std::{
    mem::ManuallyDrop,
    sync::{atomic::AtomicU64, Arc},
};

use minifb::{Window, WindowOptions};
use resource_managers::{
    PipelineManager, RenderPipelineDescriptor, RenderPipelineHandle, ShaderEntryPoint,
};
use wgpu_error_handling::{ErrorTracker, WgpuErrorScope};

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;

struct Application<'a> {
    window: Window,
    surface: ManuallyDrop<wgpu::Surface<'a>>,
    surface_format: wgpu::TextureFormat,
    adapter: wgpu::Adapter,
    device: Arc<wgpu::Device>,
    queue: wgpu::Queue,

    active_frame_index: u64,
    frame_index_for_uncaptured_errors: Arc<AtomicU64>,
    pipeline_manager: PipelineManager,
    triangle_render_pipeline: RenderPipelineHandle,
    error_tracker: Arc<ErrorTracker>,
}

impl Drop for Application<'_> {
    fn drop(&mut self) {
        // Drop surface before dropping the window, to ensure it always refers to valid window handles.
        unsafe {
            ManuallyDrop::drop(&mut self.surface);
        }
    }
}

impl<'a> Application<'a> {
    /// Initializes the application.
    ///
    /// There's various ways for this to fail, all of which are handled via `expect` right now.
    /// Of course there's be better ways to handle these (e.g. show something nice on screen or try a bit harder).
    async fn new() -> Self {
        let instance =
            wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor::default())
                .await;

        let window = Window::new(
            "terrain_and_stuff",
            WIDTH,
            HEIGHT,
            WindowOptions {
                resize: true,
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });

        // Unfortunately, mini_fb's window type isn't `Send` which is required for wgpu's `WindowHandle` trait.
        // We instead have to use the unsafe variant to create a surface directly from the window handle.
        //
        // SAFETY:
        // * The window handles are valid at this point
        // * The window is guranteed to outlive the surface since we're ensuring so in `Application's` Drop impl
        let surface = unsafe {
            instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::from_window(&window)
                    .expect("Failed to create surface target."),
            )
        }
        .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("Failed to find an appropriate adapter");
        log::info!("Created wgpu adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Device"),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(), // Needed if you want to support WebGL!
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to create device");

        // Make all errors forward to the console before panicking, this way they also show up on the web!
        let error_tracker = Arc::new(ErrorTracker::default());

        // Make sure to catch all errors, never crash, and deduplicate reported errors.
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

        let mut pipeline_manager =
            PipelineManager::new().expect("Failed to create pipeline manager");

        let surface_format = Self::pick_surface_format(&surface, &adapter);
        let triangle_render_pipeline =
            Self::create_render_pipeline(&mut pipeline_manager, &device, surface_format);

        let mut application = Application {
            window,
            surface: ManuallyDrop::new(surface),
            surface_format,
            adapter,
            device: Arc::new(device),
            queue,

            active_frame_index: 0,
            error_tracker,
            frame_index_for_uncaptured_errors,
            pipeline_manager,
            triangle_render_pipeline,
        };

        // Initial surface configuration - required at least on web since otherwise first get_current_texture() will panic.
        application.configure_surface();

        application
    }

    fn pick_surface_format(
        surface: &wgpu::Surface,
        adapter: &wgpu::Adapter,
    ) -> wgpu::TextureFormat {
        // WebGPU doesn't support sRGB(-converting-on-write) output formats, but on native the first format is often an sRGB one.
        // So if we just blindly pick the first, we'll end up with different colors!
        // Since all the colors used in this example are _already_ in sRGB, pick the first non-sRGB format!
        let surface_capabilitites = surface.get_capabilities(adapter);
        for format in &surface_capabilitites.formats {
            if !format.is_srgb() {
                return *format;
            }
        }

        log::warn!("Couldn't find a non-sRGB format, defaulting to the first one");
        surface_capabilitites.formats[0]
    }

    fn create_render_pipeline(
        pipeline_manager: &mut PipelineManager,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) -> RenderPipelineHandle {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        pipeline_manager
            .create_render_pipeline(
                device,
                RenderPipelineDescriptor {
                    debug_label: "triangle".to_owned(),
                    layout: pipeline_layout,
                    vertex_shader: ShaderEntryPoint {
                        path: "shader.wgsl".into(),
                        function_name: "vs_main".to_owned(),
                    },
                    fragment_shader: ShaderEntryPoint {
                        path: "shader.wgsl".into(),
                        function_name: "fs_main".to_owned(),
                    },
                    fragment_targets: vec![target_format.into()],
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                },
            )
            .unwrap()
    }

    fn configure_surface(&mut self) {
        // Need to reconfigure the surface and try again.
        let (width, height) = self.window.get_size();
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                format: self.surface_format,
                ..self
                    .surface
                    .get_default_config(&self.adapter, width as u32, height as u32)
                    .expect("Surface is not supported by the active adapter.")
            },
        );
    }

    pub fn update(&mut self) {
        self.active_frame_index += 1;
        self.pipeline_manager.reload_changed_pipelines(&self.device);
    }

    pub fn draw(&mut self) {
        let error_scope = WgpuErrorScope::start(&self.device);

        let frame = match self.surface.get_current_texture() {
            Ok(surface_texture) => surface_texture,
            Err(err) => match err {
                wgpu::SurfaceError::Timeout => {
                    log::warn!("Surface texture acquisition timed out.");
                    return; // Try again next frame. TODO: does this make always sense?
                }
                wgpu::SurfaceError::Outdated => {
                    // Need to reconfigure the surface and try again.
                    self.configure_surface();
                    return;
                }
                wgpu::SurfaceError::Lost => {
                    log::error!("Swapchain has been lost.");
                    return; // Try again next frame. TODO: does this make always sense?
                }
                wgpu::SurfaceError::OutOfMemory => panic!("Out of memory on surface acquisition"),
            },
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main encoder"),
            });

        if let Some(pipeline) = self
            .pipeline_manager
            .get_render_pipeline(self.triangle_render_pipeline)
        {
            let cornflower_blue = wgpu::Color {
                r: 0.39215686274509803,
                g: 0.5843137254901961,
                b: 0.9294117647058824,
                a: 1.0,
            };

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(cornflower_blue),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(pipeline);
            rpass.draw(0..3, 0..1);
        }

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
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    return; // Not used on web, this method is merely a placeholder.

    #[cfg(not(target_arch = "wasm32"))]
    main_desktop::main_desktop();
}
