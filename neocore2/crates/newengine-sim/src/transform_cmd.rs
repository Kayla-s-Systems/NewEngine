#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_math::{Quat, Vec3};
use newengine_transform_api::{
    write_entity_local_from_world_pose_local_chain, Transform, TransformDirty,
};

use crate::commands::{Command, CommandBuffer};

const MASK_POS: u8 = 1;
const MASK_ROT: u8 = 2;
const MASK_SCALE: u8 = 4;

#[derive(Clone, Copy, Debug, Default)]
struct TransformPatch {
    mask: u8,
    pos: Vec3,
    rot: Quat,
    scale: Vec3,
}

impl TransformPatch {
    #[inline]
    fn set_pos(pos: Vec3) -> Self {
        Self {
            mask: MASK_POS,
            pos,
            rot: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    #[inline]
    fn set_rot(rot: Quat) -> Self {
        Self {
            mask: MASK_ROT,
            pos: Vec3::ZERO,
            rot,
            scale: Vec3::ONE,
        }
    }

    #[inline]
    fn set_pos_rot(pos: Vec3, rot: Quat) -> Self {
        Self {
            mask: MASK_POS | MASK_ROT,
            pos,
            rot,
            scale: Vec3::ONE,
        }
    }

    #[inline]
    fn set_scale(scale: Vec3) -> Self {
        Self {
            mask: MASK_SCALE,
            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            scale,
        }
    }
}

struct PatchLocalTransformCmd {
    entity: EntityId,
    patch: TransformPatch,
}

impl Command for PatchLocalTransformCmd {
    #[inline]
    fn apply(self: Box<Self>, world: &mut World) {
        if !world.exists(self.entity) {
            return;
        }

        let Some(t) = world.get_mut_tracked::<Transform>(self.entity) else {
            return;
        };

        if (self.patch.mask & MASK_POS) != 0 {
            t.position = self.patch.pos;
        }
        if (self.patch.mask & MASK_ROT) != 0 {
            t.rotation = self.patch.rot.normalize_or_identity();
        }
        if (self.patch.mask & MASK_SCALE) != 0 {
            t.scale = self.patch.scale;
        }

        let _ = world.insert(self.entity, TransformDirty);
    }

    #[cfg(debug_assertions)]
    #[inline]
    fn tag(&self) -> crate::commands::CommandTag {
        crate::commands::CommandTag::TransformWrite
    }
}

struct SetLocalFromWorldPoseCmd {
    entity: EntityId,
    world_pos: Vec3,
    world_rot: Quat,
}

impl Command for SetLocalFromWorldPoseCmd {
    #[inline]
    fn apply(self: Box<Self>, world: &mut World) {
        if !world.exists(self.entity) {
            return;
        }

        write_entity_local_from_world_pose_local_chain(
            world,
            self.entity,
            self.world_pos,
            self.world_rot,
        );

        let _ = world.insert(self.entity, TransformDirty);
    }

    #[cfg(debug_assertions)]
    #[inline]
    fn tag(&self) -> crate::commands::CommandTag {
        crate::commands::CommandTag::TransformWrite
    }
}

/// Transform-specific command helpers.
///
/// These are the only sanctioned write paths to `Transform` inside deterministic simulation stages.
/// They preserve untouched fields and mark `TransformDirty` for derived propagation.
pub trait TransformCommandBufferExt {
    fn transform_set_local_position(&mut self, entity: EntityId, position: Vec3);
    fn transform_set_local_rotation(&mut self, entity: EntityId, rotation: Quat);
    fn transform_set_local_pose(&mut self, entity: EntityId, position: Vec3, rotation: Quat);
    fn transform_set_local_scale(&mut self, entity: EntityId, scale: Vec3);

    /// Sets local transform so the resulting world pose matches the provided world pose.
    ///
    /// Parent-local conversion is derived from the current local chain.
    fn transform_set_world_pose(&mut self, entity: EntityId, world_pos: Vec3, world_rot: Quat);
}

impl TransformCommandBufferExt for CommandBuffer {
    #[inline]
    fn transform_set_local_position(&mut self, entity: EntityId, position: Vec3) {
        self.push(Box::new(PatchLocalTransformCmd {
            entity,
            patch: TransformPatch::set_pos(position),
        }));
    }

    #[inline]
    fn transform_set_local_rotation(&mut self, entity: EntityId, rotation: Quat) {
        self.push(Box::new(PatchLocalTransformCmd {
            entity,
            patch: TransformPatch::set_rot(rotation),
        }));
    }

    #[inline]
    fn transform_set_local_pose(&mut self, entity: EntityId, position: Vec3, rotation: Quat) {
        self.push(Box::new(PatchLocalTransformCmd {
            entity,
            patch: TransformPatch::set_pos_rot(position, rotation),
        }));
    }

    #[inline]
    fn transform_set_local_scale(&mut self, entity: EntityId, scale: Vec3) {
        self.push(Box::new(PatchLocalTransformCmd {
            entity,
            patch: TransformPatch::set_scale(scale),
        }));
    }

    #[inline]
    fn transform_set_world_pose(&mut self, entity: EntityId, world_pos: Vec3, world_rot: Quat) {
        self.push(Box::new(SetLocalFromWorldPoseCmd {
            entity,
            world_pos,
            world_rot,
        }));
    }
}
