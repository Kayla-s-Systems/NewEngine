use serde::{Deserialize, Serialize};

use crate::{PhysicsEntityKey, PhysicsQuat, PhysicsVec3};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhysicsCommandKindDto {
    SetBodyPose {
        entity: PhysicsEntityKey,
        position: PhysicsVec3,
        rotation: PhysicsQuat,
    },
    SetLinearVelocity {
        entity: PhysicsEntityKey,
        velocity: PhysicsVec3,
    },
    DestroyBody {
        entity: PhysicsEntityKey,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsCommandDto {
    pub seq: u64,
    pub kind: PhysicsCommandKindDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhysicsQueryKindDto {
    Ray {
        origin: PhysicsVec3,
        dir: PhysicsVec3,
        max_t: f32,
    },
    Sphere {
        center: PhysicsVec3,
        radius: f32,
    },
    Aabb {
        min: PhysicsVec3,
        max: PhysicsVec3,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsQueryDto {
    pub seq: u64,
    pub kind: PhysicsQueryKindDto,
}
