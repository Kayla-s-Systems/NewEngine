use serde::{Deserialize, Serialize};

use crate::{
    PhysicsCommandDto, PhysicsEntityKey, PhysicsFrameBodySnapshot, PhysicsFrameColliderSnapshot,
    PhysicsQuat, PhysicsQueryDto, PhysicsVec3,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsFrameInput {
    pub frame_index: u64,
    pub fixed_tick: u64,
    pub dt: f32,
    pub gravity: f32,
    pub contact_skin: f32,
    #[serde(default)]
    pub bodies: Vec<PhysicsFrameBodySnapshot>,
    #[serde(default)]
    pub colliders: Vec<PhysicsFrameColliderSnapshot>,
    #[serde(default)]
    pub commands: Vec<PhysicsCommandDto>,
    #[serde(default)]
    pub queries: Vec<PhysicsQueryDto>,
}

impl PhysicsFrameInput {
    #[inline]
    pub fn empty(frame_index: u64, fixed_tick: u64, dt: f32) -> Self {
        Self {
            frame_index,
            fixed_tick,
            dt,
            gravity: 9.81,
            contact_skin: 0.035,
            bodies: Vec::new(),
            colliders: Vec::new(),
            commands: Vec::new(),
            queries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBodyPoseUpdate {
    pub entity: PhysicsEntityKey,
    pub position: PhysicsVec3,
    pub rotation: PhysicsQuat,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBodyVelocityUpdate {
    pub entity: PhysicsEntityKey,
    pub linear_velocity: PhysicsVec3,
    #[serde(default)]
    pub angular_velocity: PhysicsVec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsContactEventDto {
    pub a: PhysicsEntityKey,
    pub b: PhysicsEntityKey,
    pub point: PhysicsVec3,
    pub normal: PhysicsVec3,
    pub impulse: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhysicsEventDto {
    ContactBegin(PhysicsContactEventDto),
    ContactPersist(PhysicsContactEventDto),
    ContactEnd {
        a: PhysicsEntityKey,
        b: PhysicsEntityKey,
    },
    BodyCreated {
        entity: PhysicsEntityKey,
    },
    BodyDestroyed {
        entity: PhysicsEntityKey,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsQueryHitDto {
    pub seq: u64,
    pub entity: PhysicsEntityKey,
    pub position: PhysicsVec3,
    pub normal: PhysicsVec3,
    pub distance: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsStepReportDto {
    pub fixed_tick: u64,
    pub dt: f32,
    pub substeps: u32,
    pub active_bodies: usize,
    pub static_bodies: usize,
    pub dynamic_bodies: usize,
    pub contacts: usize,
    pub commands_applied: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsFrameOutput {
    pub fixed_tick: u64,
    #[serde(default)]
    pub pose_updates: Vec<PhysicsBodyPoseUpdate>,
    #[serde(default)]
    pub velocity_updates: Vec<PhysicsBodyVelocityUpdate>,
    #[serde(default)]
    pub events: Vec<PhysicsEventDto>,
    #[serde(default)]
    pub query_hits: Vec<PhysicsQueryHitDto>,
    pub report: PhysicsStepReportDto,
}
