#![forbid(unsafe_op_in_unsafe_fn)]

use super::shadows::{LightShadowPlan, ShadowFrame};
use super::super::controller::RuntimeRenderController;

#[inline]
fn shadow_frames_match_sample_space(a: ShadowFrame, b: ShadowFrame) -> bool {
    if a.texture != b.texture || a.cascade_count != b.cascade_count {
        return false;
    }
    let count = a.cascade_count.clamp(1, super::shadows::MAX_DIRECTIONAL_SHADOW_CASCADES as u32) as usize;
    for i in 0..count {
        if a.cascade_light_mvp[i] != b.cascade_light_mvp[i] {
            return false;
        }
        if (a.cascade_splits[i] - b.cascade_splits[i]).abs() > 0.001 {
            return false;
        }
    }
    a.params == b.params && a.extra == b.extra
}

impl RuntimeRenderController {
    #[inline]
    pub(super) fn should_render_shadow_map_this_frame(&mut self, plan: LightShadowPlan) -> bool {
        if !plan.is_active() {
            self.shadows.cache_valid = false;
            self.shadows.last_refresh_frame = 0;
            self.shadows.current_caster_cull = None;
            self.shadows.cached_shadow_frame = None;
            return false;
        }

        if !self.shadows.cache_valid {
            if self.shadows.warmup_defer_frames_remaining > 0 {
                self.shadows.warmup_defer_frames_remaining =
                    self.shadows.warmup_defer_frames_remaining.saturating_sub(1);
                log::debug!(
                    "render shadow cache: deferred cold shadow pass remaining={} frame={}",
                    self.shadows.warmup_defer_frames_remaining,
                    self.frame.frame_index
                );
                return false;
            }
            return true;
        }

        let period = self.shadows.refresh_period_frames.max(1);
        let frames_since_refresh = self
            .frame
            .frame_index
            .saturating_sub(self.shadows.last_refresh_frame);
        let shadow_projection_changed = self
            .shadows
            .cached_shadow_frame
            .map(|last| !shadow_frames_match_sample_space(last, plan.frame))
            .unwrap_or(true);
        if shadow_projection_changed {
            log::debug!(
                "render shadow cache: live refresh because shadow sample space changed frame={} frames_since_refresh={} period_frames={}",
                self.frame.frame_index,
                frames_since_refresh,
                period,
            );
            return true;
        }

        frames_since_refresh >= period
    }

    #[inline]
    pub(super) fn mark_shadow_map_rendered(&mut self, plan: LightShadowPlan) {
        self.shadows.cache_valid = true;
        self.shadows.last_refresh_frame = self.frame.frame_index;
        self.shadows.cached_shadow_frame = Some(plan.frame);
    }

    #[inline]
    pub(super) fn cached_shadow_frame(&self) -> Option<super::shadows::ShadowFrame> {
        self.shadows.cached_shadow_frame
    }

    #[inline]
    pub(super) fn invalidate_shadow_cache(&mut self) {
        self.shadows.cache_valid = false;
        self.shadows.last_refresh_frame = 0;
        self.shadows.warmup_defer_frames_remaining =
            super::super::render_quality::SHADOW_WARMUP_DEFER_FRAMES;
        self.shadows.current_caster_cull = None;
        self.shadows.cached_shadow_frame = None;
    }
}

impl RuntimeRenderController {
    #[inline]
    pub(super) fn set_shadow_caster_cull(&mut self, cull: Option<super::shadows::ShadowCasterCull>) {
        self.shadows.current_caster_cull = cull;
    }

    #[inline]
    pub(super) fn shadows_current_cull(&self) -> Option<super::shadows::ShadowCasterCull> {
        self.shadows.current_caster_cull
    }
}
