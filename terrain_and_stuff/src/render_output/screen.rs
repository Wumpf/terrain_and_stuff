/// Manages the target surface.
// TODO: also handle screenshotting in here
pub struct Screen<'a> {
    resolution: glam::UVec2,

    surface: wgpu::Surface<'a>,
    surface_format: wgpu::TextureFormat,
}

impl<'a> Screen<'a> {
    const PRESENT_MODE: wgpu::PresentMode = wgpu::PresentMode::AutoVsync;

    pub fn new(
        device: &wgpu::Device,
        adapter: &wgpu::Adapter,
        surface: wgpu::Surface<'a>,
        initial_resolution: glam::UVec2,
    ) -> Self {
        let surface_format = pick_surface_format(&surface, adapter);

        let mut screen = Screen {
            resolution: initial_resolution,

            surface,
            surface_format,
        };
        screen.configure_surface(device, initial_resolution);
        screen
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.resolution.x as f32 / self.resolution.y as f32
    }

    pub fn resolution(&self) -> glam::UVec2 {
        self.resolution
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    pub fn on_resize(&mut self, device: &wgpu::Device, new_resolution: glam::UVec2) {
        self.configure_surface(device, new_resolution);
    }

    pub fn start_frame(&mut self, device: &wgpu::Device) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            Ok(surface_texture) => Some(surface_texture),
            Err(err) => {
                match err {
                    wgpu::SurfaceError::Timeout => {
                        log::warn!("Surface texture acquisition timed out.");
                        // Try again next frame. TODO: does this make always sense?
                    }
                    wgpu::SurfaceError::Outdated => {
                        // Need to reconfigure the surface and try again next frame.
                        self.configure_surface(device, self.resolution);
                    }
                    wgpu::SurfaceError::Lost => {
                        log::error!("Swapchain has been lost.");
                        // Try again next frame. TODO: does this make always sense?
                    }
                    wgpu::SurfaceError::OutOfMemory => {
                        panic!("Out of memory on surface acquisition")
                    }
                }
                None
            }
        }
    }

    fn configure_surface(&mut self, device: &wgpu::Device, new_resolution: glam::UVec2) {
        self.resolution = new_resolution;
        let (width, height) = new_resolution.into();
        self.surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                format: self.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                width,
                height,
                desired_maximum_frame_latency: 2,
                present_mode: Self::PRESENT_MODE,
                alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: vec![],
            },
        );
    }
}

fn pick_surface_format(surface: &wgpu::Surface, adapter: &wgpu::Adapter) -> wgpu::TextureFormat {
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
