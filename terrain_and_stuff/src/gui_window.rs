use bytemuck::Contiguous as _;
use egui::{NumExt as _, Widget};

use crate::{
    atmosphere::{Atmosphere, AtmosphereDebugDrawMode, AtmosphereParams},
    camera::Camera,
};

pub fn run_gui(
    egui_ctx: &egui::Context,
    last_gpu_profiler_results: &[Vec<wgpu_profiler::GpuTimerQueryResult>],
    atmosphere: &mut Atmosphere,
    camera: &mut Camera,
    uses_cursor: &mut bool,
) {
    let response = egui::Window::new("Controls").show(egui_ctx, |ui| {
        egui::CollapsingHeader::new("Atmosphere")
            .default_open(true)
            .show(ui, |ui| {
                atmosphere_settings(ui, atmosphere);
            });

        egui::CollapsingHeader::new("Camera")
            .default_open(false)
            .show(ui, |ui| {
                egui::Grid::new("camera").show(ui, |ui| {
                    ui.label("Position ");
                    drag_value_vec3(ui, &mut camera.position);
                    ui.end_row();

                    ui.label("Speed: ");
                    egui::DragValue::new(&mut camera.movement_speed).ui(ui);
                    ui.end_row();
                });
            });

        egui::CollapsingHeader::new("GPU Profiling")
            .default_open(true)
            .show(ui, |ui| {
                let Some(last_result) = last_gpu_profiler_results.last() else {
                    ui.label("No profiling results available");
                    return;
                };

                // TODO: Make use of more results.
                list_gpu_profiling_results_recursive(ui, last_result);
            });
    });

    *uses_cursor =
        egui_ctx.is_using_pointer() || response.is_some_and(|r| r.response.contains_pointer());
}

fn atmosphere_settings(ui: &mut egui::Ui, atmosphere: &mut Atmosphere) {
    let AtmosphereParams {
        draw_mode,
        ground_radius_km,
        atmosphere_radius_km,
        rayleigh_scale_height,
        rayleigh_scattering_per_km_density,
        mie_scale_height,
        mie_scattering_per_km_density,
        mie_absorption_per_km_density,
        sun_disk_diameteter_rad,
        sun_disk_illuminance_factor,
        ozone_absorption_per_km_density,
        sun_illuminance,
    } = &mut atmosphere.parameters;

    let default_params = AtmosphereParams::default();

    egui::Grid::new("atmosphere_grid").show(ui, |ui| {
        ui.label("Sun azimuth");
        ui.drag_angle(&mut atmosphere.sun_azimuth);
        ui.end_row();

        ui.label("Sun altitude");
        ui.drag_angle(&mut atmosphere.sun_altitude);
        ui.end_row();

        let mut draw_mode_enum = draw_mode.get();
        ui.label("Debug draw mode");
        egui::ComboBox::from_id_salt("atmosphere_debug_draw_mode")
            .selected_text(format!("{:?}", draw_mode_enum))
            .show_ui(ui, |ui| {
                for variant_val in
                    AtmosphereDebugDrawMode::MIN_VALUE..=AtmosphereDebugDrawMode::MAX_VALUE
                {
                    let variant = AtmosphereDebugDrawMode::from_integer(variant_val).unwrap();
                    ui.selectable_value(&mut draw_mode_enum, variant, variant.to_string());
                }
            });
        draw_mode.set(draw_mode_enum);
        ui.end_row();
    });

    egui::CollapsingHeader::new("Advanced")
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("advanced_atmosphere").show(ui, |ui| {
                ui.label("Ground radius (km)");
                with_default(
                    ui,
                    ground_radius_km,
                    default_params.ground_radius_km,
                    |ui, v| {
                        ui.add(egui::DragValue::new(v));
                    },
                );
                ui.end_row();

                ui.label("Atmosphere radius (km)");
                with_default(
                    ui,
                    atmosphere_radius_km,
                    default_params.atmosphere_radius_km,
                    |ui, v| {
                        ui.add(egui::DragValue::new(v));
                    },
                );
                ui.end_row();

                ui.separator();
                ui.end_row();

                ui.label("Sun illuminance");
                with_default(
                    ui,
                    sun_illuminance,
                    default_params.sun_illuminance,
                    drag_value_vec3_precise_positive,
                );
                ui.end_row();

                ui.label("Sun disk diameter");
                with_default(
                    ui,
                    sun_disk_diameteter_rad,
                    default_params.sun_disk_diameteter_rad,
                    egui::Ui::drag_angle,
                );
                ui.end_row();

                ui.label("Sun disk illuminance factor");
                with_default(
                    ui,
                    sun_disk_illuminance_factor,
                    default_params.sun_disk_illuminance_factor,
                    drag_value_f32_precise_positive,
                );
                ui.end_row();

                ui.separator();
                ui.end_row();

                ui.label("Rayleigh scale height");
                with_default(
                    ui,
                    rayleigh_scale_height,
                    default_params.rayleigh_scale_height,
                    drag_value_f32_precise_positive,
                );
                ui.end_row();

                ui.label("Rayleigh scattering density/km");
                with_default(
                    ui,
                    rayleigh_scattering_per_km_density,
                    default_params.rayleigh_scattering_per_km_density,
                    drag_value_vec3_precise_positive,
                );
                ui.end_row();

                ui.label("Mie scale height");
                with_default(
                    ui,
                    mie_scale_height,
                    default_params.mie_scale_height,
                    drag_value_f32_precise_positive,
                );
                ui.end_row();

                ui.label("Mie scattering density/km");
                with_default(
                    ui,
                    mie_scattering_per_km_density,
                    default_params.mie_scattering_per_km_density,
                    drag_value_f32_precise_positive,
                );
                ui.end_row();

                ui.label("Mie absorption density/km");
                with_default(
                    ui,
                    mie_absorption_per_km_density,
                    default_params.mie_absorption_per_km_density,
                    drag_value_f32_precise_positive,
                );
                ui.end_row();

                ui.label("Ozone absorption density/km");
                with_default(
                    ui,
                    ozone_absorption_per_km_density,
                    default_params.ozone_absorption_per_km_density,
                    drag_value_vec3_precise_positive,
                );
                ui.end_row();
            });
        });
}

fn drag_value_vec3<T: Into<glam::Vec3> + From<glam::Vec3> + Copy>(ui: &mut egui::Ui, v: &mut T) {
    let mut vec: glam::Vec3 = (*v).into();
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut vec.x));
        ui.add(egui::DragValue::new(&mut vec.y));
        ui.add(egui::DragValue::new(&mut vec.z));
    });
    *v = T::from(vec);
}

fn drag_value_vec3_precise_positive<T: Into<glam::Vec3> + From<glam::Vec3> + Copy>(
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

fn drag_value_f32_precise_positive(ui: &mut egui::Ui, v: &mut f32) {
    let speed = (v.abs() * 0.01).at_least(0.00001);
    ui.add(
        egui::DragValue::new(v)
            .min_decimals(4)
            .max_decimals(16)
            .range(0.0..=f32::MAX)
            .speed(speed),
    );
}

fn with_default<T: Copy, R>(
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

fn list_gpu_profiling_results_recursive(
    ui: &mut egui::Ui,
    last_gpu_profiler_results: &[wgpu_profiler::GpuTimerQueryResult],
) {
    for query in last_gpu_profiler_results {
        let label = if let Some(time) = &query.time {
            format!(
                "{:02.4} ms - {}",
                (time.end - time.start) * 1000.0,
                query.label
            )
        } else {
            query.label.to_string()
        };

        if query.nested_queries.is_empty() {
            ui.label(label);
        } else {
            egui::CollapsingHeader::new(label)
                .id_salt(&query.label)
                .default_open(true)
                .show(ui, |ui| {
                    list_gpu_profiling_results_recursive(ui, &query.nested_queries);
                });
        }
    }
}
