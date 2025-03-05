use crate::{EncoderScope, render_output::Screen};

pub struct EguiMinifb {
    egui_ctx: egui::Context,
    renderer: egui_wgpu::Renderer,

    update_output: Option<egui::FullOutput>,

    mouse_state: MouseState,
}

struct MouseState {
    pos: egui::Pos2,
    left: bool,
    middle: bool,
    right: bool,
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

            mouse_state: MouseState {
                pos: egui::Pos2::ZERO,
                left: false,
                middle: false,
                right: false,
            },
        }
    }

    pub fn update(&mut self, window: &minifb::Window, run_ui: impl FnMut(&egui::Context)) {
        let window_size = window.get_size();

        let new_mouse_state = MouseState {
            pos: window
                .get_mouse_pos(minifb::MouseMode::Clamp)
                .map(|(x, y)| egui::Pos2::new(x as f32, y as f32))
                .unwrap_or_default(),
            left: window.get_mouse_down(minifb::MouseButton::Left),
            middle: window.get_mouse_down(minifb::MouseButton::Middle),
            right: window.get_mouse_down(minifb::MouseButton::Right),
        };

        let mut events = Vec::new();
        if self.mouse_state.pos != new_mouse_state.pos {
            events.push(egui::Event::PointerMoved(new_mouse_state.pos));
        }
        if self.mouse_state.left != new_mouse_state.left {
            events.push(egui::Event::PointerButton {
                pos: new_mouse_state.pos,
                button: egui::PointerButton::Primary,
                pressed: new_mouse_state.left,
                modifiers: egui::Modifiers::default(),
            });
        }
        if self.mouse_state.middle != new_mouse_state.middle {
            events.push(egui::Event::PointerButton {
                pos: new_mouse_state.pos,
                button: egui::PointerButton::Middle,
                pressed: new_mouse_state.middle,
                modifiers: egui::Modifiers::default(),
            });
        }
        if self.mouse_state.right != new_mouse_state.right {
            events.push(egui::Event::PointerButton {
                pos: new_mouse_state.pos,
                button: egui::PointerButton::Secondary,
                pressed: new_mouse_state.right,
                modifiers: egui::Modifiers::default(),
            });
        }

        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_max(
                egui::Pos2::ZERO,
                egui::pos2(window_size.0 as f32, window_size.1 as f32),
            )),
            events,
            ..egui::RawInput::default()
        };

        self.update_output = Some(self.egui_ctx.run(input, run_ui));
        self.mouse_state = new_mouse_state;
    }

    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut EncoderScope<'_>,
        render_pass: &mut wgpu_profiler::OwningScope<'_, wgpu::RenderPass<'static>>,
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
