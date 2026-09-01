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

/// Provider-neutral device-loss classification.
///
/// The runtime deliberately recognizes only the stable semantic phrase/code. Any
/// native API error mapping belongs to the provider, which should surface
/// `render.backend.device_lost` or an equivalent `device_lost` diagnostic.
#[inline]
pub(crate) fn is_backend_device_lost_error(error: &EngineError) -> bool {
    let mut text = error.to_string();
    text.make_ascii_lowercase();
    text.contains("device lost")
        || text.contains("device has been lost")
        || text.contains("device_lost")
        || text.contains("render.backend.device_lost")
}

/// Returns true for a transient render-material failure caused by an async
/// shader compile job that has been queued through `engine.threading` but has not
/// admitted backend shader code into the renderer cache yet.
///
/// This is not a fatal backend error: the next frames must keep pumping jobs and
/// retry pipeline admission instead of permanently disabling the playable viewport.
pub(crate) fn is_transient_shader_pipeline_error(error: &EngineError) -> bool {
    let mut text = error.to_string();
    text.make_ascii_lowercase();
    (text.contains("shader compile queued")
        || text.contains("shader compile pending")
        || text.contains("shader pending")
        || text.contains("shader is not ready yet")
        || text.contains("shader compile job is still pending")
        || text.contains("engine.threading shader admission timeout")
        || text.contains("leave_pending_and_retry_later")
        || text.contains("pipeline pending_event")
        || text.contains("pipeline warmup pending")
        || text.contains("bounded loading-frame work"))
        && !is_backend_device_lost_error(error)
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
            if self.backend_execution.can_recover_device_loss() {
                newengine_ulog_api::ulog::warn!(
                    "render controller: provider-owned device recovery requested phase='{}' frames_in_flight={} resource_replay={} err='{}'; route remains active for retry",
                    phase,
                    self.backend_execution.normalized_frames_in_flight(),
                    self.backend_execution.recovery.resource_replay,
                    error
                );
                newengine_core::crash::record_breadcrumb(format!(
                    "render controller: provider-owned device recovery phase='{}' err='{}'",
                    phase, error
                ));
                return Ok(());
            }

            let first = self.backend_failure.mark_disabled(phase, &error);
            self.viewport.pass_disabled = true;

            if first {
                newengine_ulog_api::ulog::error!(
                    "render controller: backend disabled after non-recoverable device loss phase='{}' policy={:?} err='{}'",
                    phase,
                    self.backend_execution.device_loss,
                    error
                );
                newengine_core::crash::record_breadcrumb(format!(
                    "render controller: backend disabled after non-recoverable device loss phase='{}' err='{}'",
                    phase, error
                ));
                newengine_ui_client::publish_render_backend_error_modal(phase, &error.to_string());
            }

            // This provider declared device loss non-recoverable in-place. Keep the
            // process/event loop alive, quiesce rendering, and leave replacement to
            // composition/hot-reload rather than guessing a backend-specific recovery path.
            return Ok(());
        }

        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_pipeline_warmup_is_retryable_not_fatal() {
        let error = EngineError::other(
            "render material registry: pipeline warmup pending cache_key='scene=Bgra8Unorm|shadow=R32Float' policy='bounded loading-frame work'",
        );
        assert!(is_transient_shader_pipeline_error(&error));
    }

    #[test]
    fn device_loss_is_never_classified_as_transient() {
        let error = EngineError::other("pipeline warmup pending while render backend device_lost");
        assert!(!is_transient_shader_pipeline_error(&error));
    }

    #[test]
    fn native_api_names_are_not_required_for_device_loss_classification() {
        let error = EngineError::other("render.backend.device_lost: provider quiesced");
        assert!(is_backend_device_lost_error(&error));
    }
}
