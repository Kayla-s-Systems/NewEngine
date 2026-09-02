#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_math::{Quat, Vec3};

use crate::commands::{Command, CommandBuffer};
use crate::{
    AngularVelocity, CameraRigComp, CharacterFacingTurnStepRequest, CharacterMotor,
    FollowTargetCameraMotor, OrbitCameraMotor, TransformCommandBufferExt, Velocity,
};

/// Deterministic controller intent.
///
/// Controllers are allowed to emit only these semantic writes. A dedicated apply stage translates
/// them into ECS/storage mutations in a single, ordered place.
#[derive(Clone, Copy, Debug)]
pub enum Intent {
    TransformSetLocalPosition {
        entity: EntityId,
        position: Vec3,
    },
    TransformSetLocalRotation {
        entity: EntityId,
        rotation: Quat,
    },
    TransformSetLocalPose {
        entity: EntityId,
        position: Vec3,
        rotation: Quat,
    },
    TransformSetLocalScale {
        entity: EntityId,
        scale: Vec3,
    },
    TransformSetWorldPose {
        entity: EntityId,
        position: Vec3,
        rotation: Quat,
    },
    SetVelocity {
        entity: EntityId,
        value: Velocity,
    },
    SetAngularVelocity {
        entity: EntityId,
        value: AngularVelocity,
    },
    SetCharacterMotor {
        entity: EntityId,
        value: CharacterMotor,
    },
    SetOrbitCameraMotor {
        entity: EntityId,
        value: OrbitCameraMotor,
    },
    SetCameraRig {
        entity: EntityId,
        value: CameraRigComp,
    },
    SetFollowTargetCameraMotor {
        entity: EntityId,
        value: FollowTargetCameraMotor,
    },
    /// Consumes one bounded presentation-authored turn-in-place step after the
    /// controller has folded it into the current frame's facing intent.
    ConsumeCharacterFacingTurnStepRequest {
        entity: EntityId,
    },
}

impl Intent {
    #[inline]
    pub fn apply_to(&self, cmd: &mut CommandBuffer) {
        match *self {
            Intent::TransformSetLocalPosition { entity, position } => {
                cmd.transform_set_local_position(entity, position);
            }
            Intent::TransformSetLocalRotation { entity, rotation } => {
                cmd.transform_set_local_rotation(entity, rotation);
            }
            Intent::TransformSetLocalPose {
                entity,
                position,
                rotation,
            } => {
                cmd.transform_set_local_pose(entity, position, rotation);
            }
            Intent::TransformSetLocalScale { entity, scale } => {
                cmd.transform_set_local_scale(entity, scale);
            }
            Intent::TransformSetWorldPose {
                entity,
                position,
                rotation,
            } => {
                cmd.transform_set_world_pose(entity, position, rotation);
            }
            Intent::SetVelocity { entity, value } => {
                cmd.insert(entity, value);
            }
            Intent::SetAngularVelocity { entity, value } => {
                cmd.insert(entity, value);
            }
            Intent::SetCharacterMotor { entity, value } => {
                cmd.insert(entity, value);
            }
            Intent::SetOrbitCameraMotor { entity, value } => {
                cmd.insert(entity, value);
            }
            Intent::SetCameraRig { entity, value } => {
                cmd.insert(entity, value);
            }
            Intent::SetFollowTargetCameraMotor { entity, value } => {
                cmd.insert(entity, value);
            }
            Intent::ConsumeCharacterFacingTurnStepRequest { entity } => {
                cmd.remove::<CharacterFacingTurnStepRequest>(entity);
            }
        }
    }
}

/// Per-system controller intent buffer.
#[derive(Clone, Debug, Default)]
pub struct IntentBuffer {
    intents: Vec<Intent>,
}

impl IntentBuffer {
    #[inline]
    pub fn new() -> Self {
        Self {
            intents: Vec::new(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.intents.is_empty()
    }

    #[inline]
    pub fn push(&mut self, intent: Intent) {
        self.intents.push(intent);
    }

    #[inline]
    pub fn extend(&mut self, other: IntentBuffer) {
        self.intents.extend(other.intents);
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Intent> {
        self.intents.iter()
    }
}

pub trait IntentSink {
    fn emit(&mut self, intent: Intent);
}

impl IntentSink for IntentBuffer {
    #[inline]
    fn emit(&mut self, intent: Intent) {
        self.push(intent);
    }
}

/// World resource used to carry controller output between stages.
#[derive(Clone, Debug, Default)]
pub struct ControllerIntentQueue {
    intents: Vec<Intent>,
}

impl ControllerIntentQueue {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.intents.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Intent> {
        self.intents.iter()
    }

    #[inline]
    pub fn snapshot(&self) -> Vec<Intent> {
        self.intents.clone()
    }

    #[inline]
    fn append(&mut self, other: IntentBuffer) {
        self.intents.extend(other.intents);
    }

    #[inline]
    fn clear(&mut self) {
        self.intents.clear();
    }
}

struct EnqueueIntentBufferCmd {
    intents: IntentBuffer,
}

impl Command for EnqueueIntentBufferCmd {
    #[inline]
    fn apply(self: Box<Self>, world: &mut World) {
        if self.intents.is_empty() {
            return;
        }

        if world.resource::<ControllerIntentQueue>().is_none() {
            world.insert_resource(ControllerIntentQueue::default());
        }

        if let Some(queue) = world.resource_mut::<ControllerIntentQueue>() {
            queue.append(self.intents);
        }
    }

    #[cfg(debug_assertions)]
    #[inline]
    fn tag(&self) -> crate::commands::CommandTag {
        crate::commands::CommandTag::IntentQueueAppend
    }
}

struct ClearControllerIntentQueueCmd;

impl Command for ClearControllerIntentQueueCmd {
    #[inline]
    fn apply(self: Box<Self>, world: &mut World) {
        if let Some(queue) = world.resource_mut::<ControllerIntentQueue>() {
            queue.clear();
        }
    }

    #[cfg(debug_assertions)]
    #[inline]
    fn tag(&self) -> crate::commands::CommandTag {
        crate::commands::CommandTag::IntentQueueClear
    }
}

pub trait IntentCommandBufferExt {
    fn enqueue_intents(&mut self, intents: IntentBuffer);
    fn clear_controller_intents(&mut self);
}

impl IntentCommandBufferExt for CommandBuffer {
    #[inline]
    fn enqueue_intents(&mut self, intents: IntentBuffer) {
        self.push(Box::new(EnqueueIntentBufferCmd { intents }));
    }

    #[inline]
    fn clear_controller_intents(&mut self) {
        self.push(Box::new(ClearControllerIntentQueueCmd));
    }
}
