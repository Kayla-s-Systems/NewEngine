//! Directional (sun/cascade) shadow atlas admission.

use super::super::super::controller::RuntimeRenderController;
use super::super::shadows::{LightShadowPlan, ShadowFrame};
use super::compare::{shadow_frame_mismatch, ShadowFrameMismatch};

impl RuntimeRenderController {
    #[inline]
    pub(in crate::render_controller::module_impl) fn should_render_shadow_map_this_frame(
        &mut self,
        plan: LightShadowPlan,
        world: &newengine_ecs::World,
    ) -> bool {
        let caster_revision = self.observe_shadow_caster_revision(world);
        if !plan.is_active() {
            self.shadows.cache_valid = false;
            self.shadows.current_caster_cull = None;
            self.shadows.cached_shadow_frame = None;
            self.shadows.cached_caster_revision = 0;
            return false;
        }

        if !self.shadows.cache_valid {
            if self.shadows.warmup_defer_frames_remaining > 0 {
                self.shadows.warmup_defer_frames_remaining =
                    self.shadows.warmup_defer_frames_remaining.saturating_sub(1);
                newengine_ulog_api::ulog::debug!(
                    "render shadow cache: deferred cold shadow pass remaining={} frame={}",
                    self.shadows.warmup_defer_frames_remaining,
                    self.frame.frame_index
                );
                return false;
            }
            self.shadows.cache_cold_refresh_count =
                self.shadows.cache_cold_refresh_count.saturating_add(1);
            return true;
        }

        let shadow_projection_mismatch = self
            .shadows
            .cached_shadow_frame
            .map(|last| shadow_frame_mismatch(last, plan.frame))
            .unwrap_or(ShadowFrameMismatch {
                texture: true,
                ..ShadowFrameMismatch::default()
            });
        if shadow_projection_mismatch.any() {
            self.shadows.cache_projection_refresh_count = self
                .shadows
                .cache_projection_refresh_count
                .saturating_add(1);
            self.shadows.cache_projection_texture_refresh_count = self
                .shadows
                .cache_projection_texture_refresh_count
                .saturating_add(u64::from(shadow_projection_mismatch.texture));
            self.shadows.cache_projection_matrix_refresh_count = self
                .shadows
                .cache_projection_matrix_refresh_count
                .saturating_add(u64::from(shadow_projection_mismatch.matrix));
            self.shadows.cache_projection_split_refresh_count = self
                .shadows
                .cache_projection_split_refresh_count
                .saturating_add(u64::from(shadow_projection_mismatch.split));
            self.shadows.cache_projection_params_refresh_count = self
                .shadows
                .cache_projection_params_refresh_count
                .saturating_add(u64::from(shadow_projection_mismatch.params));
            self.shadows.cache_projection_extra_refresh_count = self
                .shadows
                .cache_projection_extra_refresh_count
                .saturating_add(u64::from(shadow_projection_mismatch.extra));
            return true;
        }

        if self.shadows.cached_caster_revision != caster_revision {
            self.shadows.cache_caster_refresh_count =
                self.shadows.cache_caster_refresh_count.saturating_add(1);
            return true;
        }

        self.shadows.cache_reuse_count = self.shadows.cache_reuse_count.saturating_add(1);
        false
    }

    #[inline]
    pub(in crate::render_controller::module_impl) fn mark_shadow_map_rendered(
        &mut self,
        plan: LightShadowPlan,
    ) {
        self.shadows.cache_valid = true;
        self.shadows.cached_shadow_frame = Some(plan.frame);
        self.shadows.cached_caster_revision = self.shadows.caster_revision;
    }

    #[inline]
    pub(in crate::render_controller::module_impl) fn cached_shadow_frame(
        &self,
    ) -> Option<ShadowFrame> {
        self.shadows.cached_shadow_frame
    }

    #[inline]
    pub(in crate::render_controller::module_impl) fn invalidate_shadow_cache(&mut self) {
        self.shadows.cache_valid = false;
        self.shadows.warmup_defer_frames_remaining =
            super::super::super::render_quality::SHADOW_WARMUP_DEFER_FRAMES;
        self.shadows.current_caster_cull = None;
        self.shadows.cached_shadow_frame = None;
        self.shadows.cached_caster_revision = 0;
    }
}
