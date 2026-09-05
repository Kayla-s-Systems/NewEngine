//! Local (point/spot) shadow atlas admission.
//!
//! A local atlas packs many lights and up to six faces each, so its reuse test is a
//! sample-space comparison of the whole packed frame rather than a per-cascade one.

use super::super::super::controller::RuntimeRenderController;
use super::compare::local_shadow_frames_match_sample_space;

impl RuntimeRenderController {
    #[inline]
    pub(in crate::render_controller::module_impl) fn should_render_local_shadow_map_this_frame(
        &mut self,
        plan: newengine_render_feature_api::LocalShadowPlan,
        world: &newengine_ecs::World,
    ) -> bool {
        let caster_revision = self.observe_shadow_caster_revision(world);
        if !plan.is_active() {
            self.invalidate_local_shadow_cache();
            return false;
        }
        if !self.shadows.local_cache_valid {
            self.shadows.local_cache_refresh_count =
                self.shadows.local_cache_refresh_count.saturating_add(1);
            return true;
        }
        let projection_changed = self
            .shadows
            .local_cached_shadow_frame
            .map(|last| !local_shadow_frames_match_sample_space(last, plan.frame))
            .unwrap_or(true);
        if projection_changed || self.shadows.local_cached_caster_revision != caster_revision {
            self.shadows.local_cache_refresh_count =
                self.shadows.local_cache_refresh_count.saturating_add(1);
            return true;
        }
        self.shadows.local_cache_reuse_count =
            self.shadows.local_cache_reuse_count.saturating_add(1);
        false
    }

    #[inline]
    pub(in crate::render_controller::module_impl) fn mark_local_shadow_map_rendered(
        &mut self,
        plan: newengine_render_feature_api::LocalShadowPlan,
    ) {
        self.shadows.local_cache_valid = true;
        self.shadows.local_cached_shadow_frame = Some(plan.frame);
        self.shadows.local_cached_caster_revision = self.shadows.caster_revision;
    }

    #[inline]
    pub(in crate::render_controller::module_impl) fn cached_local_shadow_frame(
        &self,
    ) -> Option<newengine_render_feature_api::LocalShadowFrame> {
        self.shadows.local_cached_shadow_frame
    }

    #[inline]
    pub(in crate::render_controller::module_impl) fn invalidate_local_shadow_cache(&mut self) {
        self.shadows.local_cache_valid = false;
        self.shadows.local_cached_shadow_frame = None;
        self.shadows.local_cached_caster_revision = 0;
    }
}
