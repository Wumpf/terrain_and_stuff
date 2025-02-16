pub struct Camera {
    position: glam::Vec3,
    world_from_view_rot: glam::Quat,
    last_mouse_pos: Option<(f32, f32)>,
}

const FOV_RADIANS: f32 = 45.0 / std::f32::consts::TAU;
const UP: glam::Vec3 = glam::Vec3::Y;

impl Camera {
    pub fn new() -> Self {
        Self {
            position: glam::vec3(0.0, 0.0, 0.0),
            world_from_view_rot: glam::Quat::IDENTITY,
            last_mouse_pos: None,
        }
    }

    pub fn update(&mut self, delta_time: f32, window: &minifb::Window) {
        // X=right, Y=up, Z=back
        let mut local_movement = glam::Vec3::ZERO;
        local_movement.z += window.is_key_down(minifb::Key::W) as i32 as f32;
        local_movement.z -= window.is_key_down(minifb::Key::S) as i32 as f32;
        local_movement.x -= window.is_key_down(minifb::Key::A) as i32 as f32;
        local_movement.x += window.is_key_down(minifb::Key::D) as i32 as f32;
        local_movement.y -= window.is_key_down(minifb::Key::Q) as i32 as f32;
        local_movement.y += window.is_key_down(minifb::Key::E) as i32 as f32;
        local_movement = local_movement.normalize_or_zero();

        let mut speed = 100.0;
        if window.is_key_down(minifb::Key::LeftShift) {
            speed *= 100.0;
        } else if window.is_key_down(minifb::Key::LeftCtrl) {
            speed *= 0.1;
        } else if window.is_key_down(minifb::Key::LeftAlt) {
            speed *= 1000.0;
        }

        let world_movement = self.world_from_view_rot * (speed * local_movement);
        self.position += world_movement * delta_time;

        let mouse_pos = window.get_unscaled_mouse_pos(minifb::MouseMode::Discard);

        if window.get_mouse_down(minifb::MouseButton::Left) {
            if let (Some(current_mouse_pos), Some(last_mouse_pos)) =
                (mouse_pos, self.last_mouse_pos)
            {
                let mouse_delta = glam::vec2(
                    current_mouse_pos.0 - last_mouse_pos.0,
                    current_mouse_pos.1 - last_mouse_pos.1,
                ) * 0.01;

                // Apply change in heading:
                self.world_from_view_rot =
                    glam::Quat::from_axis_angle(UP, mouse_delta.x) * self.world_from_view_rot;

                // We need to clamp pitch to avoid nadir/zenith singularity:
                const MAX_PITCH: f32 = 0.99 * 0.25 * std::f32::consts::TAU;
                let old_pitch = self.forward().dot(UP).clamp(-1.0, 1.0).asin();
                let new_pitch = (old_pitch - mouse_delta.y).clamp(-MAX_PITCH, MAX_PITCH);
                let pitch_delta = new_pitch - old_pitch;

                // Apply change in pitch:
                self.world_from_view_rot *= glam::Quat::from_rotation_x(-pitch_delta);

                // Avoid numeric drift:
                self.world_from_view_rot = self.world_from_view_rot.normalize();
            }
        }
        self.last_mouse_pos = mouse_pos;
    }

    pub fn view_from_world(&self) -> glam::Affine3A {
        glam::Affine3A::look_to_lh(self.position, self.forward(), UP)
    }

    pub fn projection_from_view(&self, aspect_ratio: f32) -> glam::Mat4 {
        glam::Mat4::perspective_infinite_reverse_lh(FOV_RADIANS, aspect_ratio, 0.1)
    }

    pub fn position(&self) -> glam::Vec3 {
        self.position
    }

    pub fn forward(&self) -> glam::Vec3 {
        self.world_from_view_rot * glam::Vec3::Z
    }

    pub fn tan_half_fov(&self, aspect_ratio: f32) -> glam::Vec2 {
        glam::vec2(
            (FOV_RADIANS * 0.5).tan() * aspect_ratio,
            (FOV_RADIANS * 0.5).tan(),
        )
    }
}
