use newengine_entity_api::EntityHandle;
use newengine_math::{Quat, Vec3};

use crate::{PhysicsBodyDesc, PhysicsHandle};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsWorldDesc {
    pub gravity: Vec3,
    pub fixed_dt: f32,
    pub max_substeps: u32,
    pub contact_skin: f32,
    pub deterministic_order: bool,
}

impl Default for PhysicsWorldDesc {
    #[inline]
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            fixed_dt: 1.0 / 60.0,
            max_substeps: 4,
            contact_skin: 0.035,
            deterministic_order: true,
        }
    }
}

impl PhysicsWorldDesc {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            gravity: if self.gravity.is_finite() { self.gravity } else { Vec3::new(0.0, -9.81, 0.0) },
            fixed_dt: self.fixed_dt.clamp(1.0 / 240.0, 1.0 / 15.0),
            max_substeps: self.max_substeps.clamp(1, 16),
            contact_skin: self.contact_skin.clamp(0.0, 0.5),
            deterministic_order: self.deterministic_order,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicsCommandKind {
    CreateBody { entity: EntityHandle, desc: PhysicsBodyDesc, position: Vec3, rotation: Quat },
    DestroyBody { entity: EntityHandle },
    SetBodyPose { handle: PhysicsHandle, position: Vec3, rotation: Quat },
    SetLinearVelocity { handle: PhysicsHandle, velocity: Vec3 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsCommand {
    pub seq: u64,
    pub kind: PhysicsCommandKind,
}
