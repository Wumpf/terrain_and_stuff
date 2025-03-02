mod error_tracker;
mod now_or_never;
mod wgpu_error_scope;

#[cfg(not(target_arch = "wasm32"))]
mod wgpu_core_error;

pub use error_tracker::ErrorTracker;
use wgpu::Backend;
pub use wgpu_error_scope::WgpuErrorScope;

// -------

fn handle_async_error(
    backend_type: wgpu::Backend,
    resolve_callback: impl FnOnce(Option<wgpu::Error>) + 'static,
    error_future: impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static,
) {
    match backend_type {
        Backend::BrowserWebGpu => {
            #[cfg(target_arch = "wasm32")]
            {
                wasm_bindgen_futures::spawn_local(async move {
                    resolve_callback(error_future.await);
                });
            }
        }

        _ => {
            if let Some(error) = now_or_never::now_or_never(error_future) {
                resolve_callback(error);
            } else {
                log::error!(
                    "Expected wgpu errors to be ready immediately when using any of the wgpu-core based (native & webgl) backends."
                );
            }
        }
    }
}
