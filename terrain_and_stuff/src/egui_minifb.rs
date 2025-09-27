use std::sync::Arc;

use parking_lot::Mutex;

use crate::{EncoderScope, render_output::Screen};

pub struct EguiMinifb {
    egui_ctx: egui::Context,
    renderer: egui_wgpu::Renderer,

    update_output: Option<egui::FullOutput>,

    mouse_state: MouseState,

    pending_text_input: Arc<Mutex<String>>,
}

struct MouseState {
    pos: egui::Pos2,
    left: bool,
    middle: bool,
    right: bool,
}

struct TextCallback {
    pending_text_input: Arc<Mutex<String>>,
}

impl minifb::InputCallback for TextCallback {
    fn add_char(&mut self, uni_char: u32) {
        let mut pending_text_input = self.pending_text_input.lock();
        pending_text_input.push(uni_char as u8 as char);
    }
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

            pending_text_input: Arc::new(Mutex::new(String::new())),
        }
    }

    pub fn text_callback(&self) -> impl minifb::InputCallback + 'static {
        TextCallback {
            pending_text_input: self.pending_text_input.clone(),
        }
    }

    pub fn update(&mut self, window: &minifb::Window, run_ui: impl FnMut(&egui::Context)) {
        let window_size = window.get_size();

        let new_mouse_state = MouseState {
            pos: window
                .get_mouse_pos(minifb::MouseMode::Clamp)
                .map(|(x, y)| egui::Pos2::new(x, y))
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

        let mut modifiers = egui::Modifiers::default();
        for key in window.get_keys() {
            if let KeyMapResult::Modifier(modifier) = map_key(key) {
                modifiers = modifiers.plus(modifier)
            }
        }
        for key in window.get_keys_pressed(minifb::KeyRepeat::No) {
            if let KeyMapResult::Key(key) = map_key(key) {
                events.push(egui::Event::Key {
                    key,
                    physical_key: None, // Unimplemented
                    pressed: true,
                    repeat: false, // Let egui handle repeat.
                    modifiers,
                });
            }
        }
        for key in window.get_keys_released() {
            if let KeyMapResult::Key(key) = map_key(key) {
                events.push(egui::Event::Key {
                    key,
                    physical_key: None, // Unimplemented
                    pressed: false,
                    repeat: false, // Let egui handle repeat.
                    modifiers,
                });
            }
        }

        let mut pending_text_input = self.pending_text_input.lock();
        if !pending_text_input.is_empty() {
            events.push(egui::Event::Text(pending_text_input.clone()));
            *pending_text_input = String::new();
        }

        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_max(
                egui::Pos2::ZERO,
                egui::pos2(window_size.0 as f32, window_size.1 as f32),
            )),
            events,
            modifiers,
            focused: window.is_active(),
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

enum KeyMapResult {
    Key(egui::Key),
    Modifier(egui::Modifiers),
    Unknown,
}

fn map_key(key: minifb::Key) -> KeyMapResult {
    match key {
        minifb::Key::Key0 => KeyMapResult::Key(egui::Key::Num0),
        minifb::Key::Key1 => KeyMapResult::Key(egui::Key::Num1),
        minifb::Key::Key2 => KeyMapResult::Key(egui::Key::Num2),
        minifb::Key::Key3 => KeyMapResult::Key(egui::Key::Num3),
        minifb::Key::Key4 => KeyMapResult::Key(egui::Key::Num4),
        minifb::Key::Key5 => KeyMapResult::Key(egui::Key::Num5),
        minifb::Key::Key6 => KeyMapResult::Key(egui::Key::Num6),
        minifb::Key::Key7 => KeyMapResult::Key(egui::Key::Num7),
        minifb::Key::Key8 => KeyMapResult::Key(egui::Key::Num8),
        minifb::Key::Key9 => KeyMapResult::Key(egui::Key::Num9),

        minifb::Key::A => KeyMapResult::Key(egui::Key::A),
        minifb::Key::B => KeyMapResult::Key(egui::Key::B),
        minifb::Key::C => KeyMapResult::Key(egui::Key::C),
        minifb::Key::D => KeyMapResult::Key(egui::Key::D),
        minifb::Key::E => KeyMapResult::Key(egui::Key::E),
        minifb::Key::F => KeyMapResult::Key(egui::Key::F),
        minifb::Key::G => KeyMapResult::Key(egui::Key::G),
        minifb::Key::H => KeyMapResult::Key(egui::Key::H),
        minifb::Key::I => KeyMapResult::Key(egui::Key::I),
        minifb::Key::J => KeyMapResult::Key(egui::Key::J),
        minifb::Key::K => KeyMapResult::Key(egui::Key::K),
        minifb::Key::L => KeyMapResult::Key(egui::Key::L),
        minifb::Key::M => KeyMapResult::Key(egui::Key::M),
        minifb::Key::N => KeyMapResult::Key(egui::Key::N),
        minifb::Key::O => KeyMapResult::Key(egui::Key::O),
        minifb::Key::P => KeyMapResult::Key(egui::Key::P),
        minifb::Key::Q => KeyMapResult::Key(egui::Key::Q),
        minifb::Key::R => KeyMapResult::Key(egui::Key::R),
        minifb::Key::S => KeyMapResult::Key(egui::Key::S),
        minifb::Key::T => KeyMapResult::Key(egui::Key::T),
        minifb::Key::U => KeyMapResult::Key(egui::Key::U),
        minifb::Key::V => KeyMapResult::Key(egui::Key::V),
        minifb::Key::W => KeyMapResult::Key(egui::Key::W),
        minifb::Key::X => KeyMapResult::Key(egui::Key::X),
        minifb::Key::Y => KeyMapResult::Key(egui::Key::Y),
        minifb::Key::Z => KeyMapResult::Key(egui::Key::Z),
        minifb::Key::F1 => KeyMapResult::Key(egui::Key::F1),
        minifb::Key::F2 => KeyMapResult::Key(egui::Key::F2),
        minifb::Key::F3 => KeyMapResult::Key(egui::Key::F3),
        minifb::Key::F4 => KeyMapResult::Key(egui::Key::F4),
        minifb::Key::F5 => KeyMapResult::Key(egui::Key::F5),
        minifb::Key::F6 => KeyMapResult::Key(egui::Key::F6),
        minifb::Key::F7 => KeyMapResult::Key(egui::Key::F7),
        minifb::Key::F8 => KeyMapResult::Key(egui::Key::F8),
        minifb::Key::F9 => KeyMapResult::Key(egui::Key::F9),
        minifb::Key::F10 => KeyMapResult::Key(egui::Key::F10),
        minifb::Key::F11 => KeyMapResult::Key(egui::Key::F11),
        minifb::Key::F12 => KeyMapResult::Key(egui::Key::F12),
        minifb::Key::F13 => KeyMapResult::Key(egui::Key::F13),
        minifb::Key::F14 => KeyMapResult::Key(egui::Key::F14),
        minifb::Key::F15 => KeyMapResult::Key(egui::Key::F15),
        minifb::Key::Down => KeyMapResult::Key(egui::Key::ArrowDown),
        minifb::Key::Left => KeyMapResult::Key(egui::Key::ArrowLeft),
        minifb::Key::Right => KeyMapResult::Key(egui::Key::ArrowRight),
        minifb::Key::Up => KeyMapResult::Key(egui::Key::ArrowUp),
        minifb::Key::Apostrophe => KeyMapResult::Key(egui::Key::Quote),
        minifb::Key::Backquote => KeyMapResult::Key(egui::Key::Backtick),
        minifb::Key::Backslash => KeyMapResult::Key(egui::Key::Backslash),
        minifb::Key::Comma => KeyMapResult::Key(egui::Key::Comma),
        minifb::Key::Equal => KeyMapResult::Key(egui::Key::Equals),
        minifb::Key::LeftBracket => KeyMapResult::Key(egui::Key::OpenBracket),
        minifb::Key::Minus => KeyMapResult::Key(egui::Key::Minus),
        minifb::Key::Period => KeyMapResult::Key(egui::Key::Period),
        minifb::Key::RightBracket => KeyMapResult::Key(egui::Key::CloseBracket),
        minifb::Key::Semicolon => KeyMapResult::Key(egui::Key::Semicolon),
        minifb::Key::Slash => KeyMapResult::Key(egui::Key::Slash),
        minifb::Key::Backspace => KeyMapResult::Key(egui::Key::Backspace),
        minifb::Key::Delete => KeyMapResult::Key(egui::Key::Delete),
        minifb::Key::End => KeyMapResult::Key(egui::Key::End),
        minifb::Key::Enter => KeyMapResult::Key(egui::Key::Enter),
        minifb::Key::Escape => KeyMapResult::Key(egui::Key::Escape),
        minifb::Key::Home => KeyMapResult::Key(egui::Key::Home),
        minifb::Key::Insert => KeyMapResult::Key(egui::Key::Insert),

        minifb::Key::Menu => KeyMapResult::Unknown,

        minifb::Key::PageDown => KeyMapResult::Key(egui::Key::PageDown),
        minifb::Key::PageUp => KeyMapResult::Key(egui::Key::PageUp),

        minifb::Key::Pause => KeyMapResult::Unknown,

        minifb::Key::Space => KeyMapResult::Key(egui::Key::Space),
        minifb::Key::Tab => KeyMapResult::Key(egui::Key::Tab),

        minifb::Key::NumLock => KeyMapResult::Unknown,
        minifb::Key::CapsLock => KeyMapResult::Unknown,
        minifb::Key::ScrollLock => KeyMapResult::Unknown,

        minifb::Key::LeftShift => KeyMapResult::Modifier(egui::Modifiers::SHIFT),
        minifb::Key::RightShift => KeyMapResult::Modifier(egui::Modifiers::SHIFT),
        minifb::Key::LeftCtrl => KeyMapResult::Modifier(egui::Modifiers::CTRL),
        minifb::Key::RightCtrl => KeyMapResult::Modifier(egui::Modifiers::CTRL),

        minifb::Key::NumPad0 => KeyMapResult::Key(egui::Key::Num0),
        minifb::Key::NumPad1 => KeyMapResult::Key(egui::Key::Num1),
        minifb::Key::NumPad2 => KeyMapResult::Key(egui::Key::Num2),
        minifb::Key::NumPad3 => KeyMapResult::Key(egui::Key::Num3),
        minifb::Key::NumPad4 => KeyMapResult::Key(egui::Key::Num4),
        minifb::Key::NumPad5 => KeyMapResult::Key(egui::Key::Num5),
        minifb::Key::NumPad6 => KeyMapResult::Key(egui::Key::Num6),
        minifb::Key::NumPad7 => KeyMapResult::Key(egui::Key::Num7),
        minifb::Key::NumPad8 => KeyMapResult::Key(egui::Key::Num8),
        minifb::Key::NumPad9 => KeyMapResult::Key(egui::Key::Num9),
        minifb::Key::NumPadDot => KeyMapResult::Key(egui::Key::Period),
        minifb::Key::NumPadSlash => KeyMapResult::Key(egui::Key::Slash),
        minifb::Key::NumPadAsterisk => KeyMapResult::Unknown, // TODO???
        minifb::Key::NumPadMinus => KeyMapResult::Key(egui::Key::Minus),
        minifb::Key::NumPadPlus => KeyMapResult::Key(egui::Key::Plus),
        minifb::Key::NumPadEnter => KeyMapResult::Key(egui::Key::Enter),
        minifb::Key::LeftAlt => KeyMapResult::Modifier(egui::Modifiers::ALT),
        minifb::Key::RightAlt => KeyMapResult::Modifier(egui::Modifiers::ALT),

        minifb::Key::LeftSuper => KeyMapResult::Modifier(egui::Modifiers::MAC_CMD), // Note quite..
        minifb::Key::RightSuper => KeyMapResult::Modifier(egui::Modifiers::MAC_CMD),

        minifb::Key::Unknown => KeyMapResult::Unknown,
        minifb::Key::Count => KeyMapResult::Unknown,
    }
}
