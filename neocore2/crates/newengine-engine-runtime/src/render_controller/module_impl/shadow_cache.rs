#![forbid(unsafe_op_in_unsafe_fn)]

use super::shadows::LightShadowPlan;
use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    #[inline]
    pub(super) fn should_render_shadow_map_this_frame(&mut self, plan: LightShadowPlan) -> bool {
        if !plan.is_active() {
            self.shadow_cache_valid = false;
            self.shadow_last_refresh_frame = 0;
            return false;
        }

        if !self.shadow_cache_valid {
            if self.shadow_warmup_defer_frames_remaining > 0 {
                self.shadow_warmup_defer_frames_remaining =
                    self.shadow_warmup_defer_frames_remaining.saturating_sub(1);
                log::debug!(
                    "render shadow cache: deferred cold shadow pass remaining={} frame={}",
                    self.shadow_warmup_defer_frames_remaining,
                    self.frame_index
                );
                return false;
            }
            return true;
        }

        let period = self.shadow_refresh_period_frames.max(1);
        self.frame_index.saturating_sub(self.shadow_last_refresh_frame) >= period
    }

    #[inline]
    pub(super) fn mark_shadow_map_rendered(&mut self) {
        self.shadow_cache_valid = true;
        self.shadow_last_refresh_frame = self.frame_index;
    }

    #[inline]
    pub(super) fn invalidate_shadow_cache(&mut self) {
        self.shadow_cache_valid = false;
        self.shadow_last_refresh_frame = 0;
        self.shadow_warmup_defer_frames_remaining =
            super::super::render_quality::SHADOW_WARMUP_DEFER_FRAMES;
    }
}
