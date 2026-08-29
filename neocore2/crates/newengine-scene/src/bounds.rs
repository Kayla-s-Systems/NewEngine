#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_bounds::{aabb_to_sphere, sphere_to_aabb, Aabb, Bounds, BoundsKind, Sphere};
use newengine_ecs::{EntityId, World};
use newengine_math::collections_prelude::NeKey;
use newengine_transform::propagate_transforms;
use newengine_transform_api::{GlobalTransform, Parent, Transform, TransformDirty};

/// Cached scene bounds (union of all `Bounds` world-space data).
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneBounds {
    pub aabb: Option<Aabb>,
    pub sphere: Option<Sphere>,
}

/// Cache for derived scene state.
///
/// We store "last processed tick" per stage to avoid doing expensive work every frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneDerivedCache {
    pub last_transform_tick: u64,
    pub last_bounds_tick: u64,
    pub last_union_tick: u64,
}

/// Reusable scratch buffers for bounds update and union computations.
#[derive(Default)]
struct SceneBoundsScratch {
    ids: Vec<EntityId>,
}

#[inline]
fn any_transform_inputs_dirty(world: &World, since_tick: u64) -> bool {
    if world.query::<TransformDirty>().next().is_some() {
        return true;
    }

    if world.any_changed_since::<Transform>(since_tick)
        || world.any_added_since::<Transform>(since_tick)
    {
        return true;
    }

    if world.any_changed_since::<Parent>(since_tick) || world.any_added_since::<Parent>(since_tick)
    {
        return true;
    }

    false
}

#[inline]
fn any_bounds_inputs_dirty(world: &World, since_tick: u64) -> bool {
    if world.any_changed_since::<GlobalTransform>(since_tick)
        || world.any_added_since::<GlobalTransform>(since_tick)
    {
        return true;
    }

    if world.any_changed_since::<Bounds>(since_tick) || world.any_added_since::<Bounds>(since_tick)
    {
        return true;
    }

    false
}

#[inline]
fn update_bounds_from_global_transform(world: &mut World) {
    // Move scratch out to avoid borrow conflicts with world queries/gets.
    let mut scratch = core::mem::take(world.resource_mut_or_insert_default::<SceneBoundsScratch>());

    scratch.ids.clear();
    scratch
        .ids
        .extend(world.query2_ids::<GlobalTransform, Bounds>());
    scratch.ids.sort_unstable_by_key(|id| id.data().as_ffi());
    scratch.ids.dedup();

    for id in scratch.ids.iter().copied() {
        let m = match world.get::<GlobalTransform>(id) {
            Some(g) => g.0,
            None => continue,
        };

        let src = match world.get::<Bounds>(id) {
            Some(v) => *v,
            None => continue,
        };

        let mut world_sphere = src.local_sphere;
        world_sphere.center = m.transform_point3(src.local_sphere.center);

        let sx = m.x_axis.truncate().length();
        let sy = m.y_axis.truncate().length();
        let sz = m.z_axis.truncate().length();
        let s = sx.max(sy).max(sz);
        world_sphere.radius = src.local_sphere.radius * s;

        let mut world_aabb = src.local_aabb.transformed(m);
        if src.kind == BoundsKind::Sphere {
            world_aabb = sphere_to_aabb(world_sphere);
        }

        if let Some(dst) = world.get_mut::<Bounds>(id) {
            let changed = dst.world_aabb != world_aabb || dst.world_sphere != world_sphere;
            if changed {
                dst.world_aabb = world_aabb;
                dst.world_sphere = world_sphere;
                world.mark_changed::<Bounds>(id);
            }
        }
    }

    *world.resource_mut_or_insert_default::<SceneBoundsScratch>() = scratch;
}

/// Computes union world AABB for all entities that have `Bounds`.
///
/// Deterministic and allocation-free.
#[inline]
pub fn scene_world_bounds(world: &World) -> Option<Aabb> {
    let mut it = world.query::<Bounds>();
    let (_id0, b0) = it.next()?;
    let mut acc = b0.world_aabb;
    for (_id, b) in it {
        acc = acc.union(&b.world_aabb);
    }
    Some(acc)
}

/// Computes union world AABB for the provided entities (selection).
///
/// Caller controls determinism via iterator ordering.
#[inline]
pub fn selection_world_bounds(
    world: &World,
    entities: impl Iterator<Item = EntityId>,
) -> Option<Aabb> {
    let mut it = entities.filter_map(|id| world.get::<Bounds>(id).map(|b| b.world_aabb));
    let first = it.next()?;
    let mut acc = first;
    for a in it {
        acc = acc.union(&a);
    }
    Some(acc)
}

/// Updates derived scene state:
/// - propagates `Transform` -> `GlobalTransform`/`WorldPose` (dirty-gated)
/// - updates `Bounds` world_* from `GlobalTransform` (dirty-gated)
/// - caches union bounds as `SceneBounds` (dirty-gated)
#[inline]
pub fn update_scene_world(world: &mut World) {
    // Ensure core resources exist.
    let _ = world.resource_mut_or_insert_default::<SceneDerivedCache>();
    let _ = world.resource_mut_or_insert_default::<SceneBounds>();
    let _ = world.resource_mut_or_insert_default::<SceneBoundsScratch>();

    let tick_now = world.tick();

    // Stage 1: transforms.
    let last_transform_tick = {
        world
            .resource_mut_or_insert_default::<SceneDerivedCache>()
            .last_transform_tick
    };
    let mut ran_transforms = false;

    if last_transform_tick == 0 || any_transform_inputs_dirty(world, last_transform_tick) {
        propagate_transforms(world);
        ran_transforms = true;
        world
            .resource_mut_or_insert_default::<SceneDerivedCache>()
            .last_transform_tick = tick_now;
    }

    // Stage 2: bounds derived from GlobalTransform.
    let last_bounds_tick = {
        world
            .resource_mut_or_insert_default::<SceneDerivedCache>()
            .last_bounds_tick
    };
    let mut ran_bounds = false;

    let bounds_dirty =
        last_bounds_tick == 0 || ran_transforms || any_bounds_inputs_dirty(world, last_bounds_tick);
    if bounds_dirty {
        update_bounds_from_global_transform(world);
        ran_bounds = true;
        world
            .resource_mut_or_insert_default::<SceneDerivedCache>()
            .last_bounds_tick = tick_now;
    }

    // Stage 3: cache union.
    let last_union_tick = {
        world
            .resource_mut_or_insert_default::<SceneDerivedCache>()
            .last_union_tick
    };
    let union_dirty = last_union_tick == 0
        || ran_bounds
        || world.entities_changed_since(last_union_tick)
        || world.any_changed_since::<Bounds>(last_union_tick)
        || world.any_added_since::<Bounds>(last_union_tick);

    if union_dirty {
        let aabb = scene_world_bounds(world);
        let sphere = aabb.map(aabb_to_sphere);

        let sb = world.resource_mut_or_insert_default::<SceneBounds>();
        sb.aabb = aabb;
        sb.sphere = sphere;

        world
            .resource_mut_or_insert_default::<SceneDerivedCache>()
            .last_union_tick = tick_now;
    }
}

/// Returns cached scene bounds.
#[inline]
pub fn scene_bounds_cached(world: &World) -> SceneBounds {
    world.resource::<SceneBounds>().copied().unwrap_or_default()
}
