use std::collections::HashMap;

use parking_lot::Mutex;

use super::handle_async_error;

#[cfg(not(target_arch = "wasm32"))]
type ContextError = super::wgpu_core_error::WgpuCoreWrappedContextError;

#[cfg(target_arch = "wasm32")]
type ContextError = String;

pub struct ErrorEntry {
    /// Frame index for frame on which this error was last logged.
    last_occurred_frame_index: u64,

    /// Description of the error.
    #[allow(dead_code)]
    description: String,
}

/// Keeps track of wgpu errors and de-duplicates messages across frames.
///
/// On native & webgl, what accounts for as an error duplicate is a heuristic based on wgpu-core error type.
///
/// Used to avoid spamming the user with repeating errors.
/// The application maintains a "top level" error tracker for all otherwise unhandled errors.
#[derive(Default)]
pub struct ErrorTracker {
    pub errors: Mutex<HashMap<ContextError, ErrorEntry>>,
}

impl ErrorTracker {
    /// Called by the renderer context when the last error scope of a frame has finished.
    ///
    /// Error scopes live on the device timeline, which may be arbitrarily delayed compared to the content timeline.
    /// See <https://www.w3.org/TR/webgpu/#programming-model-timelines>.
    /// Do *not* call this with the content pipeline's frame index!
    pub fn on_device_timeline_frame_finished(&self, device_timeline_frame_index: u64) {
        let mut errors = self.errors.lock();
        errors.retain(|_error, entry| {
            // If the error was not logged on the just concluded frame, remove it.
            device_timeline_frame_index == entry.last_occurred_frame_index
        });
    }

    /// Handles an async error, calling [`ErrorTracker::handle_error`] as needed.
    ///
    /// `on_last_scope_resolved` is called when the last scope has resolved.
    ///
    /// `frame_index` should be the currently active frame index which is associated with the scope.
    /// (by the time the scope finishes, the active frame index may have changed)
    pub fn handle_error_future(
        self: &std::sync::Arc<Self>,
        backend: wgpu::Backend,
        error_scope_result: impl IntoIterator<
            Item = impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static,
        >,
        frame_index: u64,
        on_last_scope_resolved: impl Fn(&Self, u64) + Send + Sync + 'static,
    ) {
        let mut error_scope_result = error_scope_result.into_iter().peekable();
        while let Some(error_future) = error_scope_result.next() {
            if error_scope_result.peek().is_none() {
                let err_tracker = self.clone();
                handle_async_error(
                    backend,
                    move |error| {
                        if let Some(error) = error {
                            err_tracker.handle_error(error, frame_index);
                        }
                        on_last_scope_resolved(&err_tracker, frame_index);
                    },
                    error_future,
                );
                break;
            }

            let err_tracker = self.clone();
            handle_async_error(
                backend,
                move |error| {
                    if let Some(error) = error {
                        err_tracker.handle_error(error, frame_index);
                    }
                },
                error_future,
            );
        }
    }

    /// Logs a wgpu error to the tracker.
    ///
    /// If the error happened already already, it will be deduplicated.
    ///
    /// `frame_index` should be the frame index associated with the error scope.
    /// Since errors are reported on the `device timeline`, not the `content timeline`,
    /// this may not be the currently active frame index!
    pub fn handle_error(&self, error: wgpu::Error, frame_index: u64) {
        let is_internal_error = matches!(error, wgpu::Error::Internal { .. });

        match error {
            wgpu::Error::OutOfMemory { source: _ } => {
                log::error!("A wgpu operation caused out-of-memory: {error}");
            }
            wgpu::Error::Internal {
                source: _source,
                description,
            }
            | wgpu::Error::Validation {
                source: _source,
                description,
            } => {
                let entry = ErrorEntry {
                    last_occurred_frame_index: frame_index,
                    description: description.clone(),
                };

                #[cfg(not(target_arch = "wasm32"))]
                let ctx_err = {
                    let ctx_err = _source
                        .downcast::<wgpu::wgc::error::ContextError>()
                        .expect("Expected wgpu core error");
                    if ctx_err
                        .source
                        .downcast_ref::<wgpu::wgc::command::CommandEncoderError>()
                        .is_some()
                    {
                        // Actual command encoder errors never carry any meaningful
                        // information: ignore them.
                        return;
                    }

                    super::wgpu_core_error::WgpuCoreWrappedContextError(ctx_err)
                };
                #[cfg(target_arch = "wasm32")]
                let ctx_err = description.clone();

                if self.errors.lock().insert(ctx_err, entry).is_none() {
                    let base_description = if is_internal_error {
                        "Internal wgpu error"
                    } else {
                        "Wgpu validation error"
                    };
                    log::error!("{base_description} {frame_index}: {description}");
                }
            }
        }
    }
}
