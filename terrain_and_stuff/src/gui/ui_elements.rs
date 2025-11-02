use crate::wgpu_utils::wgpu_buffer_types::WgslEnum;
use egui::{NumExt as _, Ui};

pub fn drag_value_vec3<T: Into<glam::Vec3> + From<glam::Vec3> + Copy>(
    ui: &mut egui::Ui,
    v: &mut T,
) {
    let mut vec: glam::Vec3 = (*v).into();
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut vec.x));
        ui.add(egui::DragValue::new(&mut vec.y));
        ui.add(egui::DragValue::new(&mut vec.z));
    });
    *v = T::from(vec);
}

pub fn drag_value_vec3_precise_positive<T: Into<glam::Vec3> + From<glam::Vec3> + Copy>(
    ui: &mut egui::Ui,
    v: &mut T,
) {
    let mut vec: glam::Vec3 = (*v).into();
    ui.horizontal(|ui| {
        drag_value_f32_precise_positive(ui, &mut vec.x);
        drag_value_f32_precise_positive(ui, &mut vec.y);
        drag_value_f32_precise_positive(ui, &mut vec.z);
    });
    *v = T::from(vec);
}

pub fn drag_value_f32_precise_positive(ui: &mut egui::Ui, v: &mut f32) {
    let speed = (v.abs() * 0.01).at_least(0.00001);
    ui.add(
        egui::DragValue::new(v)
            .min_decimals(4)
            .max_decimals(16)
            .range(0.0..=f32::MAX)
            .speed(speed),
    );
}

/// Modify an angle. The given angle should be in radians, but is shown to the user in degrees.
/// The angle is NOT wrapped, so the user may select, for instance 720° = 2𝞃 = 4π
///
/// Adjusted version from egui for different speed & decimals.
pub fn drag_angle(ui: &mut egui::Ui, radians: &mut f32) -> egui::Response {
    let mut degrees = radians.to_degrees();
    let mut response = ui.add(
        egui::DragValue::new(&mut degrees)
            .fixed_decimals(1)
            .speed(0.1)
            .suffix("°"),
    );

    // only touch `*radians` if we actually changed the degree value
    if degrees != radians.to_degrees() {
        *radians = degrees.to_radians();
        response.mark_changed();
    }

    response
}

pub fn row_with_default<T: Copy, R>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    default: T,
    f: impl FnOnce(&mut egui::Ui, &mut T) -> R,
) -> egui::InnerResponse<R> {
    ui.label(label);
    let response = with_default(ui, value, default, f);
    ui.end_row();
    response
}

pub fn with_default<T: Copy, R>(
    ui: &mut egui::Ui,
    value: &mut T,
    default: T,
    f: impl FnOnce(&mut egui::Ui, &mut T) -> R,
) -> egui::InnerResponse<R> {
    ui.horizontal(|ui| {
        if ui.small_button("↩").clicked() {
            *value = default;
        }
        f(ui, value)
    })
}

pub fn enum_combobox<
    T: Copy
        + Into<u32>
        + bytemuck::Contiguous<Int = u32>
        + bytemuck::CheckedBitPattern
        + bytemuck::Zeroable
        + std::fmt::Display
        + PartialEq
        + 'static,
>(
    ui: &mut Ui,
    mode: &mut WgslEnum<T>,
) {
    let mut mode_enum = mode.get();
    ui.label("Debug draw mode");
    egui::ComboBox::from_id_salt("debug_draw_mode")
        .selected_text(mode_enum.to_string())
        .show_ui(ui, |ui| {
            for variant in T::MIN_VALUE..=T::MAX_VALUE {
                let variant = T::from_integer(variant).unwrap();
                ui.selectable_value(&mut mode_enum, variant, variant.to_string());
            }
        });
    mode.set(mode_enum);
}
