#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_math::{Mat4, Quat, Vec3};
use newengine_transform_api::EntityHandle;

use newengine_transform_api::{GlobalTransform, Parent, Transform};

const MAX_CHAIN: usize = 64;

#[inline]
fn resolve_transform_entity(world: &World, entity: EntityHandle) -> Option<EntityId> {
    world
        .query::<Transform>()
        .find(|(id, _)| id.stable_u64() == entity.stable_id)
        .map(|(id, _)| id)
}

#[inline]
fn world_matrix_from_local_chain(world: &World, id: EntityId) -> Option<Mat4> {
    if !world.exists(id) {
        return None;
    }

    // Collect the chain: [self, parent, grandparent, ...] up to a deterministic cap.
    // If the chain is deeper than the cap or a parent link is broken, we degrade gracefully.
    let mut chain: [EntityId; MAX_CHAIN] = [EntityId::default(); MAX_CHAIN];
    let mut len: usize = 0;

    let mut cur = id;
    loop {
        if len >= MAX_CHAIN {
            break;
        }
        chain[len] = cur;
        len += 1;

        let Some(p) = world.get::<Parent>(cur).copied() else {
            break;
        };

        // Degrade: if parent handle cannot be resolved to a transform entity, treat current as root.
        let Some(parent_id) = resolve_transform_entity(world, p.0) else {
            break;
        };

        cur = parent_id;
    }

    // Compose from root -> leaf.
    let mut m = Mat4::IDENTITY;
    for i in (0..len).rev() {
        let eid = chain[i];
        let Some(t) = world.get::<Transform>(eid).copied() else {
            // Degrade: treat missing local as identity.
            continue;
        };
        m *= t.to_mat4();
    }

    Some(m)
}

/// Reads an entity world pose (rotation + translation) from the best available source.
///
/// Priority:
/// 1) `GlobalTransform` (if present)
/// 2) Transform local-chain composition (`Transform` + `Parent`)
///
/// Notes:
/// - If you need *strictly up-to-date* world pose before derived propagation, prefer
///   [`read_entity_world_pose_local_chain`].
#[inline]
pub fn read_entity_world_pose(world: &World, id: EntityId) -> Option<(Vec3, Quat)> {
    if let Some(gt) = world.get::<GlobalTransform>(id) {
        let (_s, rot, trans) = gt.0.to_scale_rotation_translation();
        return Some((trans, rot.normalize_or_identity()));
    }

    read_entity_world_pose_local_chain(world, id)
}

/// Reads the exact world matrix by composing local transforms along the `Parent` chain.
///
/// Unlike `GlobalTransform`, this does not depend on derived propagation having run for the current
/// frame, so editor manipulations of parented entities can consume same-frame parent changes.
#[inline]
pub fn read_entity_world_matrix_local_chain(world: &World, id: EntityId) -> Option<Mat4> {
    world_matrix_from_local_chain(world, id)
}

/// Reads an entity world pose by composing local transforms along the `Parent` chain.
///
/// This function does **not** depend on derived propagation state (`GlobalTransform`) and is
/// therefore safe to use inside simulation/controller stages.
#[inline]
pub fn read_entity_world_pose_local_chain(world: &World, id: EntityId) -> Option<(Vec3, Quat)> {
    let m = world_matrix_from_local_chain(world, id)?;
    let (_s, rot, trans) = m.to_scale_rotation_translation();
    Some((trans, rot.normalize_or_identity()))
}

/// Writes an entity local `Transform` so that the resulting world pose matches `(world_pos, world_rot)`.
///
/// - If the entity has a parent chain, the world pose is converted into parent-local space.
/// - Scale is preserved from the existing local `Transform`.
///
/// This function is safe to use pre-propagation: it derives the parent world matrix from the local
/// transform chain.
#[inline]
pub fn write_entity_local_from_world_pose(
    world: &mut World,
    id: EntityId,
    world_pos: Vec3,
    world_rot: Quat,
) {
    write_entity_local_from_world_pose_local_chain(world, id, world_pos, world_rot)
}

/// Same as [`write_entity_local_from_world_pose`], but explicitly documents the local-chain policy.
#[inline]
pub fn write_entity_local_from_world_pose_local_chain(
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
        if let Some(parent_id) = resolve_transform_entity(world, p.0) {
            if let Some(pm) = world_matrix_from_local_chain(world, parent_id) {
                let local = pm.inverse() * world_m;
                let (_s, rot, trans) = local.to_scale_rotation_translation();
                t.position = trans;
                t.rotation = rot.normalize_or_identity();
                t.scale = preserve_scale;
                let _ = world.insert(id, t);
                return;
            }
        }
    }

    // No parent chain: local == world.
    t.position = world_pos;
    t.rotation = world_rot.normalize_or_identity();
    t.scale = preserve_scale;
    let _ = world.insert(id, t);
}
