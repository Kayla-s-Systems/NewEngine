#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::RenderBackendStatus;
use newengine_core::{EngineError, EngineResult};

use super::controller::RuntimeRenderController;

#[derive(Default)]
pub(crate) struct RenderBackendFailureState {
    disabled: bool,
    phase: Option<&'static str>,
    message: Option<String>,
}

impl RenderBackendFailureState {
    #[inline]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub(crate) fn is_disabled(&self) -> bool {
        self.disabled
    }


    #[inline]
    pub(crate) fn snapshot(&self) -> RenderBackendStatus {
        if self.disabled {
            RenderBackendStatus {
                degraded: true,
                phase: self.phase,
                message: self.message.clone(),
            }
        } else {
            RenderBackendStatus::healthy()
        }
    }

    fn mark_disabled(&mut self, phase: &'static str, error: &EngineError) -> bool {
        let first = !self.disabled;
        self.disabled = true;
        self.phase = Some(phase);
        self.message = Some(error.to_string());
        first
    }
}

#[inline]
pub(crate) fn is_backend_device_lost_error(error: &EngineError) -> bool {
    let mut text = error.to_string();
    text.make_ascii_lowercase();
    text.contains("device lost")
        || text.contains("device has been lost")
        || text.contains("error_device_lost")
        || text.contains("vk_error_device_lost")
        || text.contains("vulkan device lost")
}

impl RuntimeRenderController {
    #[inline]
    pub(crate) fn backend_render_disabled(&self) -> bool {
        self.backend_failure.is_disabled()
    }

    pub(crate) fn record_render_backend_error(
        &mut self,
        phase: &'static str,
        error: EngineError,
    ) -> EngineResult<()> {
        if is_backend_device_lost_error(&error) {
            let first = self.backend_failure.mark_disabled(phase, &error);
            self.viewport.pass_disabled = true;
    
            if first {
                log::error!(
                    "render controller: backend disabled after fatal GPU error phase='{}' err='{}'",
                    phase,
                    error
                );
                newengine_core::crash::record_breadcrumb(format!(
                    "render controller: backend disabled after fatal GPU error phase='{}' err='{}'",
                    phase, error
                ));
            }

            // Device loss is fatal for the backend, but not for the process. Keep the
            // platform/event loop alive and stop issuing GPU work until the app exits
            // or the renderer plugin is recreated by a future hot-reload path.
            return Ok(());
        }

        Err(error)
    }

    #[inline]
    pub(crate) fn record_render_backend_result(
        &mut self,
        phase: &'static str,
        result: EngineResult<()>,
    ) -> EngineResult<()> {
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.record_render_backend_error(phase, error),
        }
    }
}
