use egui::NumExt as _;

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
