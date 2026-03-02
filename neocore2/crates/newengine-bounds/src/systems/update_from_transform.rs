#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_math::collections_prelude::NeKey;

use crate::{sphere_to_aabb, Bounds, BoundsKind};

/// Reusable scratch buffers to avoid per-frame allocations.
#[derive(Default)]
struct BoundsUpdateFromTransformScratch {
    ids: Vec<EntityId>,
}

/// Updates world-space bounds from `newengine_transform::Transform`.
///
/// Contract:
/// - deterministic iteration (stable entity id order)
/// - no per-frame heap churn (scratch is a resource)
/// - mutations are tracked (mark changed only when derived data differs)
#[inline]
pub fn update_bounds_from_transform_system(world: &mut World) {
    // Move scratch out to avoid borrow conflicts with world queries/gets.
    let mut scratch =
        core::mem::take(world.resource_mut_or_insert_default::<BoundsUpdateFromTransformScratch>());

    scratch.ids.clear();
    scratch
        .ids
        .extend(world.query2_ids::<newengine_transform::Transform, Bounds>());
    scratch.ids.sort_unstable_by_key(|id| id.data().as_ffi());
    scratch.ids.dedup();

    for id in scratch.ids.iter().copied() {
        let t = match world.get::<newengine_transform::Transform>(id) {
            Some(v) => *v,
            None => continue,
        };

        let src = match world.get::<Bounds>(id) {
            Some(v) => *v,
            None => continue,
        };

        let m = t.matrix();

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

    *world.resource_mut_or_insert_default::<BoundsUpdateFromTransformScratch>() = scratch;
}
