//! Who casts a shadow, and how real change is detected.
//!
//! Transform propagation marks many static `GlobalTransform` components changed every
//! frame, so membership/pose/skin hashes - not ECS change ticks alone - decide whether
//! the atlas content would actually differ. `observe_shadow_caster_revision` is the one
//! entry point both the directional and the local cache consult, and it is memoised per
//! world tick so two atlases never pay for it twice in a frame.

use super::super::super::controller::RuntimeRenderController;
use newengine_gameplay_world_runtime::gameplay::DisplayVisibility;
use newengine_materials::MaterialRef;
use newengine_model_domain_api::MeshRenderOptions;
use newengine_primitives::Primitive;
use newengine_procedural_noise::ProceduralTerrain;

// Skin deformation is high-frequency visual state. Treating every published pose as a global
// atlas invalidation forced all CSM cascades to be rebuilt every render frame and defeated the
// temporal shadow cache completely. Static/membership/projection changes still invalidate
// immediately; skin-only deformation refreshes at a bounded cadence.
const DYNAMIC_SKIN_SHADOW_REFRESH_INTERVAL_FRAMES: u64 = 2;

#[inline]
pub(super) const fn dynamic_skin_shadow_refresh_due(
    frame_index: u64,
    first_observation: bool,
) -> bool {
    first_observation || frame_index.is_multiple_of(DYNAMIC_SKIN_SHADOW_REFRESH_INTERVAL_FRAMES)
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
        .get::<newengine_gameplay_world_runtime::gameplay::EnvironmentDomeRenderState>(entity)
        .is_some()
        || world
            .get::<DisplayVisibility>(entity)
            .is_some_and(|visibility| !visibility.visible_in_game())
    {
        return false;
    }

    if world.get::<Primitive>(entity).is_some()
        || world
            .get::<newengine_gameplay_world_runtime::gameplay::ModelRenderComponent>(entity)
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
fn quantized_shadow_pose_component(value: f32) -> u64 {
    if !value.is_finite() {
        return 0;
    }
    // Ignore transform-system float noise below 0.1 mm / equivalent matrix delta.
    // Real gameplay motion remains orders of magnitude larger and invalidates immediately.
    ((value * 10_000.0).round() as i64) as u64
}

#[inline]
fn shadow_caster_pose_hash(
    world: &newengine_ecs::World,
    caster_entities: &[newengine_ecs::EntityId],
) -> u64 {
    use newengine_math::hash_combine_u64;
    let mut xor_hash = 0_u64;
    let mut sum_hash = 0_u64;
    let mut count = 0_u64;
    for &entity in caster_entities {
        // Wind/sway for instanced foliage is intentionally shadow-stable at P0.
        if world
            .get::<MeshRenderOptions>(entity)
            .is_some_and(|options| {
                matches!(
                    options.role,
                    newengine_model_domain_api::MeshRenderRole::FoliageInstanced
                )
            })
        {
            continue;
        }
        let Some(global) = world.get::<newengine_transform::GlobalTransform>(entity) else {
            continue;
        };
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
pub(super) fn shadow_skin_pose_hash(
    world: &newengine_ecs::World,
    caster_entities: &[newengine_ecs::EntityId],
) -> u64 {
    use newengine_math::hash_combine_u64;
    let mut xor_hash = 0_u64;
    let mut sum_hash = 0_u64;
    let mut count = 0_u64;
    for &entity in caster_entities {
        let Some(skin) =
            world.get::<newengine_gameplay_world_runtime::gameplay::PlayerSkinBinding>(entity)
        else {
            continue;
        };
        let Some(pose) =
            world.get::<newengine_gameplay_world_runtime::gameplay::PlayerSkinPose>(skin.owner)
        else {
            continue;
        };
        // PlayerSkinPose.revision is the authoritative publication revision. Shadow admission
        // needs only freshness, not a second O(joints) content hash of a palette that animation
        // already produced and published. A revision change may conservatively redraw an identical
        // silhouette, but can never retain stale deformation.
        let mut h = hash_combine_u64(entity.stable_u64(), skin.owner.stable_u64());
        h = hash_combine_u64(h, pose.revision);
        h = hash_combine_u64(h, pose.palette.len() as u64);
        xor_hash ^= h.rotate_left((entity.stable_u64() & 63) as u32);
        sum_hash = sum_hash.wrapping_add(h.wrapping_mul(0xd6e8_feb8_6659_fd93));
        count = count.saturating_add(1);
    }
    hash_combine_u64(hash_combine_u64(count, xor_hash), sum_hash)
}

fn rebuild_shadow_caster_membership(
    world: &newengine_ecs::World,
    caster_entities: &mut Vec<newengine_ecs::EntityId>,
) -> u64 {
    use newengine_math::hash_combine_u64;
    use std::collections::HashSet;
    caster_entities.clear();
    let mut seen = HashSet::<u64>::new();
    let mut xor_hash = 0_u64;
    let mut sum_hash = 0_u64;
    let mut count = 0_u64;
    let mut add = |entity: newengine_ecs::EntityId, geometry_key: u64| {
        if !shadow_caster_entity(world, entity) || !seen.insert(entity.stable_u64()) {
            return;
        }
        caster_entities.push(entity);
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
    for (entity, model) in
        world.query::<newengine_gameplay_world_runtime::gameplay::ModelRenderComponent>()
    {
        add(
            entity,
            newengine_materials::api::fnv1a64(model.logical_path.as_bytes())
                ^ 0x4d4f_444c_0000_0003,
        );
    }

    hash_combine_u64(hash_combine_u64(count, xor_hash), sum_hash)
}

impl RuntimeRenderController {
    /// Caster revision for this world tick, memoised so the directional and local
    /// atlases share one observation per frame.
    #[inline]
    pub(super) fn observe_shadow_caster_revision(&mut self, world: &newengine_ecs::World) -> u64 {
        let current_tick = world.tick();
        let since_tick = self.shadows.caster_observed_tick;
        if since_tick != 0 && since_tick == current_tick {
            return self.shadows.caster_revision;
        }
        let first_observation = since_tick == 0;

        let membership_maybe_dirty = first_observation
            || world.entities_changed_since(since_tick)
            || world.any_changed_since::<Primitive>(since_tick)
            || world.any_added_since::<Primitive>(since_tick)
            || world.any_changed_since::<ProceduralTerrain>(since_tick)
            || world.any_added_since::<ProceduralTerrain>(since_tick)
            || world.any_changed_since::<newengine_gameplay_world_runtime::gameplay::ModelRenderComponent>(since_tick)
            || world.any_added_since::<newengine_gameplay_world_runtime::gameplay::ModelRenderComponent>(since_tick)
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
            let membership_hash =
                rebuild_shadow_caster_membership(world, &mut self.shadows.caster_entities);
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
        let pose_hash = shadow_caster_pose_hash(world, &self.shadows.caster_entities);
        let bounds_changed = first_observation || pose_hash != self.shadows.caster_pose_hash;
        self.shadows.caster_pose_hash = pose_hash;

        // Skin deformation changes shadow geometry without changing the entity GlobalTransform.
        // PlayerSkinPose revision is the publication freshness contract; do not re-hash hundreds
        // of already-produced matrices inside shadow admission.
        let skin_pose_hash = shadow_skin_pose_hash(world, &self.shadows.caster_entities);
        let skin_pose_differs =
            first_observation || skin_pose_hash != self.shadows.caster_skin_pose_hash;
        let skin_pose_changed = skin_pose_differs
            && dynamic_skin_shadow_refresh_due(self.frame.frame_index, first_observation);
        // Do not consume a deferred skin revision. Keeping the previous admitted hash means the
        // next cadence slot still observes the pending deformation and refreshes the atlas.
        if skin_pose_changed {
            self.shadows.caster_skin_pose_hash = skin_pose_hash;
        }

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
                            .get::<newengine_gameplay_world_runtime::gameplay::ModelRenderComponent>(entity)
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

        self.shadows.caster_observed_tick = current_tick;
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
}
