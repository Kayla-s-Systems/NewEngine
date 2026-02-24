#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_math::{Mat4, Quat, Vec3};

use crate::{GlobalTransform, Parent, Transform};

/// Reads an entity world pose (rotation + translation) from the best available source.
///
/// Priority:
/// 1) `GlobalTransform` (propagated pose)
/// 2) `Transform` (local == world if not propagated)
#[inline]
pub fn read_entity_world_pose(world: &World, id: EntityId) -> Option<(Vec3, Quat)> {
    if let Some(gt) = world.get::<GlobalTransform>(id) {
        let (_s, rot, trans) = gt.0.to_scale_rotation_translation();
        return Some((trans, rot.normalize_or_identity()));
    }

    let t = world.get::<Transform>(id).copied()?;
    Some((t.position, t.rotation.normalize_or_identity()))
}

/// Writes an entity local `Transform` so that the resulting world pose matches `(world_pos, world_rot)`.
///
/// If the entity has a parent and the parent has `GlobalTransform`, the function converts the world pose
/// into parent-local space. Scale is preserved from the existing `Transform`.
#[inline]
pub fn write_entity_local_from_world_pose(
    world: &mut World,
    id: EntityId,
    world_pos: Vec3,
    world_rot: Quat,
) {
    let Some(mut t) = world.get::<Transform>(id).copied() else {
        return;
    };

    let preserve_scale = t.scale;
    let world_m = Mat4::from_scale_rotation_translation(Vec3::ONE, world_rot, world_pos);

    if let Some(p) = world.get::<Parent>(id).copied() {
        if let Some(pgt) = world.get::<GlobalTransform>(p.0) {
            let local = pgt.0.inverse() * world_m;
            let (_s, rot, trans) = local.to_scale_rotation_translation();
            t.position = trans;
            t.rotation = rot.normalize_or_identity();
            t.scale = preserve_scale;
            let _ = world.insert(id, t);
            return;
        }
    }

    // No parent or missing parent world pose: local == world.
    t.position = world_pos;
    t.rotation = world_rot.normalize_or_identity();
    t.scale = preserve_scale;
    let _ = world.insert(id, t);
}
