use crate::render_output::Screen;

pub struct EguiMinifb {
    egui_ctx: egui::Context,
    renderer: egui_wgpu::Renderer,

    update_output: Option<egui::FullOutput>,
}

impl EguiMinifb {
    pub fn new(device: &wgpu::Device, screen: &Screen) -> Self {
        // TODO: DPI scaling, see also https://github.com/emoon/rust_minifb/issues/236

        let dithering = true;
        let msaa_samples = 1;
        let renderer = egui_wgpu::Renderer::new(
            device,
            screen.surface_format(),
            None,
            msaa_samples,
            dithering,
        );

        Self {
            egui_ctx: egui::Context::default(),
            renderer,

            update_output: None,
        }
    }

    pub fn update(&mut self, window: &minifb::Window, run_ui: impl FnMut(&egui::Context)) {
        let window_size = window.get_size();

        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_max(
                egui::Pos2::ZERO,
                egui::pos2(window_size.0 as f32, window_size.1 as f32),
            )),

            ..egui::RawInput::default()
        };
        self.update_output = Some(self.egui_ctx.run(input, run_ui));
    }

    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        render_pass: &mut wgpu::RenderPass<'static>,
    ) {
        let egui::FullOutput {
            platform_output: _,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output: _,
        } = self.update_output.take().unwrap();

        let screen_size_pixels = self.egui_ctx.screen_rect().size() * pixels_per_point;
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [screen_size_pixels.x as u32, screen_size_pixels.y as u32],
            pixels_per_point,
        };

        for (id, image_delta) in textures_delta.set {
            self.renderer
                .update_texture(device, queue, id, &image_delta);
        }

        let clipped_primitives = self.egui_ctx.tessellate(shapes, pixels_per_point);
        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        self.renderer
            .render(render_pass, &clipped_primitives, &screen_descriptor);

        for id in textures_delta.free {
            self.renderer.free_texture(&id);
        }
    }
}
