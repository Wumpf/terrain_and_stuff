mod ui_elements;

use bytemuck::Contiguous as _;
use egui::Widget as _;

use crate::{
    atmosphere::{AtmosphereDebugDrawMode, AtmosphereParams, SunAngles},
    config::Config,
    gui::ui_elements::row_with_default,
};

use ui_elements::{
    drag_angle, drag_value_f32_precise_positive, drag_value_vec3, drag_value_vec3_precise_positive,
};

pub fn run_gui(
    egui_ctx: &egui::Context,
    last_gpu_profiler_results: &[Vec<wgpu_profiler::GpuTimerQueryResult>],
    uses_cursor: &mut bool,
    config: &mut Config,
) {
    let response = egui::Window::new("Controls").show(egui_ctx, |ui| {
        if ui.button("Reset all to defaults").clicked() {
            *config = Config::default();
        }

        egui::CollapsingHeader::new("Atmosphere")
            .default_open(true)
            .show(ui, |ui| {
                atmosphere_settings(ui, &mut config.sun_angles, &mut config.atmosphere_params);
            });

        egui::CollapsingHeader::new("Camera")
            .default_open(false)
            .show(ui, |ui| {
                egui::Grid::new("camera").show(ui, |ui| {
                    ui.label("Position ");
                    drag_value_vec3(ui, &mut config.camera.position);
                    ui.end_row();

                    ui.label("Speed: ");
                    egui::DragValue::new(&mut config.camera.base_movement_speed).ui(ui);
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

fn atmosphere_settings(
    ui: &mut egui::Ui,
    sun_angles: &mut SunAngles,
    atmosphere_params: &mut AtmosphereParams,
) {
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
        enable_multiple_scattering,
        sun_illuminance,
        ground_albedo,
    } = atmosphere_params;

    let default_params = AtmosphereParams::default();

    egui::Grid::new("atmosphere_grid").show(ui, |ui| {
        let SunAngles {
            sun_azimuth,
            sun_altitude,
        } = sun_angles;

        ui.label("Sun azimuth");
        drag_angle(ui, sun_azimuth);
        ui.end_row();

        ui.label("Sun altitude");
        drag_angle(ui, sun_altitude);
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
                row_with_default(
                    ui,
                    "Ground radius (km)",
                    ground_radius_km,
                    default_params.ground_radius_km,
                    |ui, v| {
                        ui.add(egui::DragValue::new(v));
                    },
                );

                row_with_default(
                    ui,
                    "Atmosphere radius (km)",
                    atmosphere_radius_km,
                    default_params.atmosphere_radius_km,
                    |ui, v| {
                        ui.add(egui::DragValue::new(v));
                    },
                );

                ui.separator();
                ui.end_row();

                row_with_default(
                    ui,
                    "Sun illuminance",
                    sun_illuminance,
                    default_params.sun_illuminance,
                    drag_value_vec3_precise_positive,
                );

                row_with_default(
                    ui,
                    "Sun disk diameter",
                    sun_disk_diameteter_rad,
                    default_params.sun_disk_diameteter_rad,
                    egui::Ui::drag_angle,
                );

                row_with_default(
                    ui,
                    "Sun disk illuminance factor",
                    sun_disk_illuminance_factor,
                    default_params.sun_disk_illuminance_factor,
                    drag_value_f32_precise_positive,
                );

                ui.separator();
                ui.end_row();

                row_with_default(
                    ui,
                    "Enable multiple scattering",
                    enable_multiple_scattering,
                    default_params.enable_multiple_scattering,
                    |ui, v| {
                        let mut as_bool = (*v).into();
                        ui.checkbox(&mut as_bool, "");
                        *v = as_bool.into();
                    },
                );

                ui.separator();
                ui.end_row();

                row_with_default(
                    ui,
                    "Rayleigh scale height",
                    rayleigh_scale_height,
                    default_params.rayleigh_scale_height,
                    drag_value_f32_precise_positive,
                );

                row_with_default(
                    ui,
                    "Rayleigh scattering density/km",
                    rayleigh_scattering_per_km_density,
                    default_params.rayleigh_scattering_per_km_density,
                    drag_value_vec3_precise_positive,
                );

                row_with_default(
                    ui,
                    "Mie scale height",
                    mie_scale_height,
                    default_params.mie_scale_height,
                    drag_value_f32_precise_positive,
                );

                row_with_default(
                    ui,
                    "Mie scattering density/km",
                    mie_scattering_per_km_density,
                    default_params.mie_scattering_per_km_density,
                    drag_value_f32_precise_positive,
                );

                row_with_default(
                    ui,
                    "Mie absorption density/km",
                    mie_absorption_per_km_density,
                    default_params.mie_absorption_per_km_density,
                    drag_value_f32_precise_positive,
                );

                row_with_default(
                    ui,
                    "Ozone absorption density/km",
                    ozone_absorption_per_km_density,
                    default_params.ozone_absorption_per_km_density,
                    drag_value_vec3_precise_positive,
                );

                row_with_default(
                    ui,
                    "Ground albedo",
                    ground_albedo,
                    default_params.ground_albedo,
                    drag_value_vec3_precise_positive,
                );
            });
        });
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
