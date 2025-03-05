pub fn run_gui(
    egui_ctx: &egui::Context,
    last_gpu_profiler_results: &[Vec<wgpu_profiler::GpuTimerQueryResult>],
) {
    egui::Window::new("Controls").show(egui_ctx, |ui| {
        egui::CollapsingHeader::new("GPU Profiling")
            .open(Some(true))
            .show(ui, |ui| {
                let Some(last_result) = last_gpu_profiler_results.last() else {
                    ui.label("No profiling results available");
                    return;
                };

                // TODO: Make use of more results.
                list_gpu_profiling_results_recursive(ui, &last_result);
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
            format!("{}", query.label)
        };

        if query.nested_queries.is_empty() {
            ui.label(label);
        } else {
            egui::CollapsingHeader::new(label)
                .open(Some(true))
                .show(ui, |ui| {
                    list_gpu_profiling_results_recursive(ui, &query.nested_queries);
                });
        }
    }
}
