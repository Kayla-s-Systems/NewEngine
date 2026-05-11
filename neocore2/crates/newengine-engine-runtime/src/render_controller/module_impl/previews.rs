#![forbid(unsafe_op_in_unsafe_fn)]

use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(super) fn pump_previews_fail_soft(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        dt: f32,
    ) {
        if self.previews_disabled {
            return;
        }

        let result = {
            let mut previews = self.previews.lock();
            previews.pump(r, dt)
        };

        if let Err(e) = result {
            self.previews_disabled = true;
            log::warn!(
                "render controller: primitive previews disabled for this session: {}",
                e
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: primitive previews disabled: {}",
                e
            ));
        }
    }

    pub(super) fn disable_viewport_pass(&mut self, phase: &'static str, error: impl std::fmt::Display) {
        if !self.viewport_pass_disabled {
            log::error!(
                "render controller: viewport GPU pass disabled at {}: {}",
                phase,
                error
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: viewport pass disabled at {}: {}",
                phase,
                error
            ));
        }
        self.viewport_pass_disabled = true;
    }
}
