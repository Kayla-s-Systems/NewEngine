#![forbid(unsafe_op_in_unsafe_fn)]

use super::super::controller::RuntimeRenderController;
use super::shadows::{LightShadowPlan, ShadowFrame};
use crate::gameplay::DisplayVisibility;
use newengine_bounds::Bounds;
use newengine_materials::MaterialRef;
use newengine_model_domain_api::MeshRenderOptions;
use newengine_primitives::Primitive;
use newengine_procedural_noise::ProceduralTerrain;

const SHADOW_MATRIX_EPSILON: f32 = 2.0e-4;
const SHADOW_PARAM_EPSILON: f32 = 1.0e-4;
const SHADOW_SPLIT_EPSILON: f32 = 1.0e-3;

#[inline]
fn slices_nearly_equal(a: &[f32], b: &[f32], epsilon: f32) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(left, right)| (*left - *right).abs() <= epsilon)
}

#[inline]
fn shadow_matrices_match(a: newengine_math::Mat4, b: newengine_math::Mat4) -> bool {
    let a_cols = a.to_cols_array();
    let b_cols = b.to_cols_array();
    slices_nearly_equal(&a_cols, &b_cols, SHADOW_MATRIX_EPSILON)
}

#[inline]
fn shadow_frames_match_sample_space(a: ShadowFrame, b: ShadowFrame) -> bool {
    if a.texture != b.texture || a.cascade_count != b.cascade_count {
        return false;
    }
    let count = a
        .cascade_count
        .clamp(1, super::shadows::MAX_DIRECTIONAL_SHADOW_CASCADES as u32) as usize;
    for i in 0..count {
        if !shadow_matrices_match(a.cascade_light_mvp[i], b.cascade_light_mvp[i]) {
            return false;
        }
        if (a.cascade_splits[i] - b.cascade_splits[i]).abs() > SHADOW_SPLIT_EPSILON {
            return false;
        }
    }
    slices_nearly_equal(&a.params, &b.params, SHADOW_PARAM_EPSILON)
        && slices_nearly_equal(&a.extra, &b.extra, SHADOW_PARAM_EPSILON)
}

impl RuntimeRenderController {
    #[inline]
    fn observe_shadow_caster_revision(&mut self, world: &newengine_ecs::World) -> u64 {
        let since_tick = self.shadows.caster_observed_tick;
        let first_observation = since_tick == 0;
        let entity_changed = first_observation || world.entities_changed_since(since_tick);
        let bounds_changed = first_observation
            || world.any_changed_since::<Bounds>(since_tick)
            || world.any_added_since::<Bounds>(since_tick);
        // `Primitive` contains both immutable geometry identity (`id`) and mutable
        // visual color. Sky/environment animation legitimately changes color every
        // frame, so the coarse ECS changed tick is not a shadow-geometry signal.
        // Runtime geometry replacement is represented by primitive insertion/lifecycle;
        // procedural terrain has true mutable geometry and keeps full change tracking.
        let geometry_changed = first_observation
            || world.any_added_since::<Primitive>(since_tick)
            || world.any_changed_since::<ProceduralTerrain>(since_tick)
            || world.any_added_since::<ProceduralTerrain>(since_tick);
        let material_changed = first_observation
            || world.any_changed_since::<MeshRenderOptions>(since_tick)
            || world.any_added_since::<MeshRenderOptions>(since_tick)
            || world.any_changed_since::<MaterialRef>(since_tick)
            || world.any_added_since::<MaterialRef>(since_tick);
        let visibility_changed = first_observation
            || world.any_changed_since::<DisplayVisibility>(since_tick)
            || world.any_added_since::<DisplayVisibility>(since_tick);
        let changed = entity_changed
            || bounds_changed
            || geometry_changed
            || material_changed
            || visibility_changed;

        self.shadows.caster_observed_tick = world.tick();
        if changed {
            self.shadows.caster_revision = self.shadows.caster_revision.saturating_add(1).max(1);
            self.shadows.caster_entity_change_count = self
                .shadows
                .caster_entity_change_count
                .saturating_add(u64::from(entity_changed));
            self.shadows.caster_bounds_change_count = self
                .shadows
                .caster_bounds_change_count
                .saturating_add(u64::from(bounds_changed));
            self.shadows.caster_geometry_change_count = self
                .shadows
                .caster_geometry_change_count
                .saturating_add(u64::from(geometry_changed));
            self.shadows.caster_material_change_count = self
                .shadows
                .caster_material_change_count
                .saturating_add(u64::from(material_changed));
            self.shadows.caster_visibility_change_count = self
                .shadows
                .caster_visibility_change_count
                .saturating_add(u64::from(visibility_changed));
        }
        self.shadows.caster_revision
    }

    #[inline]
    pub(super) fn should_render_shadow_map_this_frame(
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

        let shadow_projection_changed = self
            .shadows
            .cached_shadow_frame
            .map(|last| !shadow_frames_match_sample_space(last, plan.frame))
            .unwrap_or(true);
        if shadow_projection_changed {
            self.shadows.cache_projection_refresh_count = self
                .shadows
                .cache_projection_refresh_count
                .saturating_add(1);
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
    pub(super) fn mark_shadow_map_rendered(&mut self, plan: LightShadowPlan) {
        self.shadows.cache_valid = true;
        self.shadows.cached_shadow_frame = Some(plan.frame);
        self.shadows.cached_caster_revision = self.shadows.caster_revision;
    }

    #[inline]
    pub(super) fn cached_shadow_frame(&self) -> Option<super::shadows::ShadowFrame> {
        self.shadows.cached_shadow_frame
    }

    #[inline]
    pub(super) fn invalidate_shadow_cache(&mut self) {
        self.shadows.cache_valid = false;
        self.shadows.warmup_defer_frames_remaining =
            super::super::render_quality::SHADOW_WARMUP_DEFER_FRAMES;
        self.shadows.current_caster_cull = None;
        self.shadows.cached_shadow_frame = None;
        self.shadows.cached_caster_revision = 0;
    }
}

impl RuntimeRenderController {
    #[inline]
    pub(super) fn set_shadow_caster_cull(
        &mut self,
        cull: Option<super::shadows::ShadowCasterCull>,
    ) {
        self.shadows.current_caster_cull = cull;
    }

    #[inline]
    pub(super) fn shadows_current_cull(&self) -> Option<super::shadows::ShadowCasterCull> {
        self.shadows.current_caster_cull
    }
}
