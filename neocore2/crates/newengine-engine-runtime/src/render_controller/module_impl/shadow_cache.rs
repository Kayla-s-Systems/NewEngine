#![forbid(unsafe_op_in_unsafe_fn)]

use super::super::controller::RuntimeRenderController;
use super::shadows::{LightShadowPlan, ShadowFrame};
use crate::gameplay::DisplayVisibility;
use newengine_materials::MaterialRef;
use newengine_model_domain_api::MeshRenderOptions;
use newengine_primitives::Primitive;
use newengine_procedural_noise::ProceduralTerrain;

// Directional sun motion is render-cadence input. A loose matrix epsilon makes the
// atlas hold several frames and then jump, which is visible as shadow stepping/flicker.
// Keep only a machine-noise guard here; static texel-snapped projections remain bit-stable.
const SHADOW_DIRECTIONAL_MATRIX_EPSILON: f32 = 1.0e-6;
// Local-light atlases retain a looser threshold because small point/spot transform noise
// would otherwise fan out into six perspective redraws per light.
const SHADOW_LOCAL_MATRIX_EPSILON: f32 = 2.0e-4;
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
fn shadow_matrices_match(a: newengine_math::Mat4, b: newengine_math::Mat4, epsilon: f32) -> bool {
    let a_cols = a.to_cols_array();
    let b_cols = b.to_cols_array();
    slices_nearly_equal(&a_cols, &b_cols, epsilon)
}

#[inline]
fn shadow_viewport_matches(
    a: newengine_core::render::Viewport,
    b: newengine_core::render::Viewport,
) -> bool {
    a.x.to_bits() == b.x.to_bits()
        && a.y.to_bits() == b.y.to_bits()
        && a.w.to_bits() == b.w.to_bits()
        && a.h.to_bits() == b.h.to_bits()
        && a.min_depth.to_bits() == b.min_depth.to_bits()
        && a.max_depth.to_bits() == b.max_depth.to_bits()
}

#[inline]
fn shadow_scissor_matches(
    a: newengine_core::render::RectI32,
    b: newengine_core::render::RectI32,
) -> bool {
    a.x == b.x && a.y == b.y && a.w == b.w && a.h == b.h
}

#[derive(Clone, Copy, Debug, Default)]
struct ShadowFrameMismatch {
    texture: bool,
    matrix: bool,
    split: bool,
    params: bool,
    extra: bool,
}

impl ShadowFrameMismatch {
    #[inline]
    fn any(self) -> bool {
        self.texture || self.matrix || self.split || self.params || self.extra
    }
}

fn shadow_frame_mismatch(a: ShadowFrame, b: ShadowFrame) -> ShadowFrameMismatch {
    let mut mismatch = ShadowFrameMismatch {
        texture: a.texture != b.texture || a.cascade_count != b.cascade_count,
        ..ShadowFrameMismatch::default()
    };
    let count = a
        .cascade_count
        .min(b.cascade_count)
        .clamp(1, super::shadows::MAX_DIRECTIONAL_SHADOW_CASCADES as u32) as usize;
    for i in 0..count {
        mismatch.matrix |= !shadow_matrices_match(
            a.cascade_light_mvp[i],
            b.cascade_light_mvp[i],
            SHADOW_DIRECTIONAL_MATRIX_EPSILON,
        );
        mismatch.split |= (a.cascade_splits[i] - b.cascade_splits[i]).abs() > SHADOW_SPLIT_EPSILON;
    }
    mismatch.params = !slices_nearly_equal(&a.params, &b.params, SHADOW_PARAM_EPSILON);
    mismatch.extra = !slices_nearly_equal(&a.extra, &b.extra, SHADOW_PARAM_EPSILON);
    mismatch
}

#[inline]
fn local_shadow_frames_match_sample_space(
    a: newengine_render_feature_api::LocalShadowFrame,
    b: newengine_render_feature_api::LocalShadowFrame,
) -> bool {
    if a.texture != b.texture
        || a.atlas_extent != b.atlas_extent
        || a.light_count != b.light_count
        || a.view_count != b.view_count
    {
        return false;
    }
    let light_count =
        a.light_count
            .min(newengine_render_feature_api::MAX_LOCAL_SHADOW_LIGHTS as u32) as usize;
    for i in 0..light_count {
        let left = a.lights[i];
        let right = b.lights[i];
        if left.stable_id != right.stable_id
            || left.light_kind != right.light_kind
            || left.packed_light_index != right.packed_light_index
            || left.first_view != right.first_view
            || left.view_count != right.view_count
            || left.resolution != right.resolution
            || (left.range - right.range).abs() > SHADOW_PARAM_EPSILON
            || (left.bias - right.bias).abs() > SHADOW_PARAM_EPSILON
            || (left.normal_bias - right.normal_bias).abs() > SHADOW_PARAM_EPSILON
            || (left.strength - right.strength).abs() > SHADOW_PARAM_EPSILON
        {
            return false;
        }
    }
    let view_count =
        a.view_count
            .min(newengine_render_feature_api::MAX_LOCAL_SHADOW_VIEWS as u32) as usize;
    for i in 0..view_count {
        let left = a.views[i];
        let right = b.views[i];
        if !shadow_matrices_match(left.light_mvp, right.light_mvp, SHADOW_LOCAL_MATRIX_EPSILON)
            || !shadow_viewport_matches(left.viewport, right.viewport)
            || !shadow_scissor_matches(left.scissor, right.scissor)
            || left.light_slot != right.light_slot
            || left.face_index != right.face_index
            || left.resolution != right.resolution
        {
            return false;
        }
    }
    true
}

#[inline]
fn render_options_cast_shadows(options: &MeshRenderOptions) -> bool {
    use newengine_model_domain_api::{MeshRenderRole, MeshShadowPolicy};
    if matches!(
        options.role,
        MeshRenderRole::SkyBackground
            | MeshRenderRole::CelestialBillboard
            | MeshRenderRole::WeatherVolume
            | MeshRenderRole::FirstPersonViewModel
            | MeshRenderRole::CollisionProxy
            | MeshRenderRole::EditorGizmo
            | MeshRenderRole::DebugPrimitive
    ) {
        return false;
    }
    matches!(
        options.shadow_policy,
        MeshShadowPolicy::CastOnly
            | MeshShadowPolicy::CastAndReceive
            | MeshShadowPolicy::ProfileControlled
    )
}

#[inline]
fn shadow_caster_entity(world: &newengine_ecs::World, entity: newengine_ecs::EntityId) -> bool {
    if world
        .get::<crate::gameplay::EnvironmentDomeRenderState>(entity)
        .is_some()
        || world
            .get::<DisplayVisibility>(entity)
            .is_some_and(|visibility| !visibility.visible_in_game())
    {
        return false;
    }

    if world.get::<Primitive>(entity).is_some()
        || world
            .get::<crate::gameplay::ModelRenderComponent>(entity)
            .is_some()
    {
        let options = world
            .get::<MeshRenderOptions>(entity)
            .cloned()
            .unwrap_or_else(MeshRenderOptions::world_opaque);
        return render_options_cast_shadows(&options);
    }

    if world.get::<ProceduralTerrain>(entity).is_some() {
        let options = world
            .get::<MeshRenderOptions>(entity)
            .cloned()
            .unwrap_or_else(MeshRenderOptions::terrain_patch);
        return render_options_cast_shadows(&options);
    }

    false
}

#[inline]
fn shadow_pose_change_invalidates(
    world: &newengine_ecs::World,
    entity: newengine_ecs::EntityId,
) -> bool {
    if !shadow_caster_entity(world, entity) {
        return false;
    }
    // Wind/sway for instanced foliage is intentionally shadow-stable at P0.
    // Rebuilding the whole directional/local atlas for every per-instance sway
    // transform destroys caching. Foliage shadows update opportunistically on
    // the next real light/caster refresh; alpha-cutout shape remains authored.
    !world
        .get::<MeshRenderOptions>(entity)
        .is_some_and(|options| {
            matches!(
                options.role,
                newengine_model_domain_api::MeshRenderRole::FoliageInstanced
            )
        })
}

#[inline]
fn quantized_shadow_pose_component(value: f32) -> u64 {
    if !value.is_finite() {
        return 0;
    }
    // Ignore transform-system float noise below 0.1 mm / equivalent matrix delta.
    // Real gameplay motion remains orders of magnitude larger and invalidates immediately.
    ((value * 10_000.0).round() as i64) as u64
}

#[inline]
fn shadow_caster_pose_hash(world: &newengine_ecs::World) -> u64 {
    use newengine_math::hash_combine_u64;
    let mut xor_hash = 0_u64;
    let mut sum_hash = 0_u64;
    let mut count = 0_u64;
    for (entity, global) in world.query::<newengine_transform::GlobalTransform>() {
        if !shadow_pose_change_invalidates(world, entity) {
            continue;
        }
        let mut h = entity.stable_u64();
        for value in global.0.to_cols_array() {
            h = hash_combine_u64(h, quantized_shadow_pose_component(value));
        }
        xor_hash ^= h.rotate_left((entity.stable_u64() & 63) as u32);
        sum_hash = sum_hash.wrapping_add(h.wrapping_mul(0x94d0_49bb_1331_11eb));
        count = count.saturating_add(1);
    }
    hash_combine_u64(hash_combine_u64(count, xor_hash), sum_hash)
}

#[inline]
fn shadow_skin_pose_hash(world: &newengine_ecs::World) -> u64 {
    use newengine_math::hash_combine_u64;
    let mut xor_hash = 0_u64;
    let mut sum_hash = 0_u64;
    let mut count = 0_u64;
    for (entity, skin) in world.query::<crate::gameplay::PlayerSkinBinding>() {
        if !shadow_caster_entity(world, entity) {
            continue;
        }
        let Some(pose) = world.get::<crate::gameplay::PlayerSkinPose>(skin.owner) else {
            continue;
        };
        // `PlayerSkinPose.revision` is a publication counter and advances every render
        // frame even when the resulting palette is numerically unchanged. Hash the
        // quantized palette itself so shadow refresh follows real deformation, not cadence.
        let mut h = hash_combine_u64(entity.stable_u64(), skin.owner.stable_u64());
        h = hash_combine_u64(h, pose.palette.len() as u64);
        for matrix in &pose.palette {
            for value in matrix.to_cols_array() {
                h = hash_combine_u64(h, quantized_shadow_pose_component(value));
            }
        }
        xor_hash ^= h.rotate_left((entity.stable_u64() & 63) as u32);
        sum_hash = sum_hash.wrapping_add(h.wrapping_mul(0xd6e8_feb8_6659_fd93));
        count = count.saturating_add(1);
    }
    hash_combine_u64(hash_combine_u64(count, xor_hash), sum_hash)
}

fn shadow_caster_membership_hash(world: &newengine_ecs::World) -> u64 {
    use newengine_math::hash_combine_u64;
    let mut xor_hash = 0_u64;
    let mut sum_hash = 0_u64;
    let mut count = 0_u64;
    let mut add = |entity: newengine_ecs::EntityId, geometry_key: u64| {
        if !shadow_caster_entity(world, entity) {
            return;
        }
        let material_key = world
            .get::<MaterialRef>(entity)
            .map(|material| material.id.raw())
            .unwrap_or(0);
        let mut h = hash_combine_u64(entity.stable_u64(), geometry_key);
        h = hash_combine_u64(h, material_key);
        xor_hash ^= h.rotate_left((entity.stable_u64() & 63) as u32);
        sum_hash = sum_hash.wrapping_add(h.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        count = count.saturating_add(1);
    };

    for (entity, primitive) in world.query::<Primitive>() {
        add(entity, primitive.id.0 ^ 0x5052_494d_0000_0001);
    }
    for (entity, terrain) in world.query::<ProceduralTerrain>() {
        add(entity, terrain.mesh_key() ^ 0x5445_5252_0000_0002);
    }
    for (entity, model) in world.query::<crate::gameplay::ModelRenderComponent>() {
        add(
            entity,
            newengine_materials::api::fnv1a64(model.logical_path.as_bytes())
                ^ 0x4d4f_444c_0000_0003,
        );
    }

    hash_combine_u64(hash_combine_u64(count, xor_hash), sum_hash)
}

impl RuntimeRenderController {
    #[inline]
    fn observe_shadow_caster_revision(&mut self, world: &newengine_ecs::World) -> u64 {
        let since_tick = self.shadows.caster_observed_tick;
        let first_observation = since_tick == 0;

        let membership_maybe_dirty = first_observation
            || world.entities_changed_since(since_tick)
            || world.any_changed_since::<Primitive>(since_tick)
            || world.any_added_since::<Primitive>(since_tick)
            || world.any_changed_since::<ProceduralTerrain>(since_tick)
            || world.any_added_since::<ProceduralTerrain>(since_tick)
            || world.any_changed_since::<crate::gameplay::ModelRenderComponent>(since_tick)
            || world.any_added_since::<crate::gameplay::ModelRenderComponent>(since_tick)
            || world.any_changed_since::<MeshRenderOptions>(since_tick)
            || world.any_added_since::<MeshRenderOptions>(since_tick)
            || world.any_changed_since::<newengine_model_domain_api::FoliageInstanceRuntime>(
                since_tick,
            )
            || world
                .any_added_since::<newengine_model_domain_api::FoliageInstanceRuntime>(since_tick)
            || world.any_changed_since::<MaterialRef>(since_tick)
            || world.any_added_since::<MaterialRef>(since_tick)
            || world.any_changed_since::<DisplayVisibility>(since_tick)
            || world.any_added_since::<DisplayVisibility>(since_tick);

        let entity_changed = if membership_maybe_dirty {
            let membership_hash = shadow_caster_membership_hash(world);
            let changed =
                first_observation || membership_hash != self.shadows.caster_membership_hash;
            self.shadows.caster_membership_hash = membership_hash;
            changed
        } else {
            false
        };

        // Transform propagation marks many static GlobalTransform components changed each
        // frame. Compare actual caster matrices instead of ECS change ticks, otherwise a
        // perfectly static scene can never reuse its shadow atlas.
        let pose_hash = shadow_caster_pose_hash(world);
        let bounds_changed = first_observation || pose_hash != self.shadows.caster_pose_hash;
        self.shadows.caster_pose_hash = pose_hash;

        // Skin deformation changes shadow geometry without changing the entity GlobalTransform.
        // Compare quantized palette content so actual deformation refreshes the atlas while a
        // publication-only revision bump cannot force a full CSM redraw.
        let skin_pose_hash = shadow_skin_pose_hash(world);
        let skin_pose_changed =
            first_observation || skin_pose_hash != self.shadows.caster_skin_pose_hash;
        self.shadows.caster_skin_pose_hash = skin_pose_hash;

        let geometry_changed = entity_changed
            || world
                .query_changed::<ProceduralTerrain>(since_tick)
                .any(|(entity, _)| shadow_caster_entity(world, entity));
        let material_changed = first_observation
            || world
                .query_changed::<MeshRenderOptions>(since_tick)
                .any(|(entity, _)| shadow_caster_entity(world, entity))
            || world
                .query_changed::<MaterialRef>(since_tick)
                .any(|(entity, _)| shadow_caster_entity(world, entity));
        let visibility_changed = first_observation
            || world
                .query_changed::<DisplayVisibility>(since_tick)
                .any(|(entity, _)| {
                    world.get::<Primitive>(entity).is_some()
                        || world.get::<ProceduralTerrain>(entity).is_some()
                        || world
                            .get::<crate::gameplay::ModelRenderComponent>(entity)
                            .is_some()
                })
            || world
                .query_changed::<newengine_model_domain_api::FoliageInstanceRuntime>(since_tick)
                .any(|(entity, _)| shadow_caster_entity(world, entity));
        let changed = entity_changed
            || bounds_changed
            || skin_pose_changed
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
    pub(super) fn should_render_local_shadow_map_this_frame(
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
    pub(super) fn mark_local_shadow_map_rendered(
        &mut self,
        plan: newengine_render_feature_api::LocalShadowPlan,
    ) {
        self.shadows.local_cache_valid = true;
        self.shadows.local_cached_shadow_frame = Some(plan.frame);
        self.shadows.local_cached_caster_revision = self.shadows.caster_revision;
    }

    #[inline]
    pub(super) fn cached_local_shadow_frame(
        &self,
    ) -> Option<newengine_render_feature_api::LocalShadowFrame> {
        self.shadows.local_cached_shadow_frame
    }

    #[inline]
    pub(super) fn invalidate_local_shadow_cache(&mut self) {
        self.shadows.local_cache_valid = false;
        self.shadows.local_cached_shadow_frame = None;
        self.shadows.local_cached_caster_revision = 0;
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

#[cfg(test)]
mod temporal_shadow_cache_tests {
    use super::*;

    #[test]
    fn directional_matrix_cache_tracks_real_sub_old_epsilon_motion() {
        let stable = newengine_math::Mat4::IDENTITY;
        let machine_noise = newengine_math::Mat4::from_translation(newengine_math::Vec3::new(
            SHADOW_DIRECTIONAL_MATRIX_EPSILON * 0.25,
            0.0,
            0.0,
        ));
        let real_motion =
            newengine_math::Mat4::from_translation(newengine_math::Vec3::new(1.0e-5, 0.0, 0.0));
        assert!(shadow_matrices_match(
            stable,
            machine_noise,
            SHADOW_DIRECTIONAL_MATRIX_EPSILON,
        ));
        assert!(!shadow_matrices_match(
            stable,
            real_motion,
            SHADOW_DIRECTIONAL_MATRIX_EPSILON,
        ));
        assert!(shadow_matrices_match(
            stable,
            real_motion,
            SHADOW_LOCAL_MATRIX_EPSILON,
        ));
    }

    #[test]
    fn skin_shadow_hash_tracks_palette_content_not_publication_revision() {
        let mut world = newengine_ecs::World::new();
        let owner = world.spawn();
        let visual = world.spawn();
        let _ = world.insert(
            owner,
            crate::gameplay::PlayerSkinPose {
                palette: vec![newengine_math::Mat4::IDENTITY],
                revision: 1,
            },
        );
        let _ = world.insert(
            visual,
            newengine_primitives::Primitive {
                id: newengine_primitives::builtins::ID_CUBE,
                color: [1.0; 4],
            },
        );
        let _ = world.insert(
            visual,
            crate::gameplay::PlayerSkinBinding {
                owner,
                vertices: Vec::new(),
                source_to_model: newengine_math::Mat4::IDENTITY.to_cols_array(),
            },
        );

        let baseline = shadow_skin_pose_hash(&world);
        world
            .get_mut::<crate::gameplay::PlayerSkinPose>(owner)
            .expect("skin pose")
            .revision = 2;
        let revision_only = shadow_skin_pose_hash(&world);
        assert_eq!(
            baseline, revision_only,
            "publication revision alone must not redraw CSM"
        );
        world
            .get_mut::<crate::gameplay::PlayerSkinPose>(owner)
            .expect("skin pose")
            .palette[0] =
            newengine_math::Mat4::from_translation(newengine_math::Vec3::new(0.02, 0.0, 0.0));
        let deformed = shadow_skin_pose_hash(&world);
        assert_ne!(
            baseline, deformed,
            "actual skin deformation must invalidate shadow geometry"
        );
    }
}
