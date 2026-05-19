#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_math::{Quat, Vec3};
use newengine_transform::{read_entity_world_pose_local_chain, Transform};

use crate::{IntentBuffer, SimFrame};

/// Read-only controller view for a single deterministic simulation tick.
///
/// Controllers may inspect the world through this context, but they must emit state changes into
/// an [`IntentBuffer`] instead of mutating ECS storages directly.
#[derive(Clone, Copy)]
pub struct ControllerCtx<'a> {
    world: &'a World,
    frame: SimFrame,
}

impl<'a> ControllerCtx<'a> {
    #[inline]
    pub fn new(world: &'a World, frame: SimFrame) -> Self {
        Self { world, frame }
    }

    #[inline]
    pub fn world(&self) -> &'a World {
        self.world
    }

    #[inline]
    pub fn frame(&self) -> SimFrame {
        self.frame
    }

    #[inline]
    pub fn dt(&self) -> f32 {
        self.frame.dt
    }

    #[inline]
    pub fn has_transform(&self, entity: EntityId) -> bool {
        self.world.get::<Transform>(entity).is_some()
    }

    #[inline]
    pub fn local_rotation_or_identity(&self, entity: EntityId) -> Quat {
        self.world
            .get::<Transform>(entity)
            .map(|t| t.rotation)
            .unwrap_or(Quat::IDENTITY)
    }

    #[inline]
    pub fn read_world_pose(&self, entity: EntityId) -> Option<(Vec3, Quat)> {
        read_entity_world_pose_local_chain(self.world, entity)
    }
}

/// Domain contract for entity-level controllers.
///
/// Concrete gameplay/editor controllers should depend on this contract layer, not on ECS mutation
/// details. All side effects must be emitted as intents.
pub trait EntityControllerV1 {
    fn update(&mut self, entity: EntityId, ctx: &ControllerCtx<'_>, out: &mut IntentBuffer);
}
