use newengine_entity_api::EntityHandle;
use newengine_math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsContactEvent {
    pub a: EntityHandle,
    pub b: EntityHandle,
    pub point: Vec3,
    pub normal: Vec3,
    pub impulse: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicsEvent {
    ContactBegin(PhysicsContactEvent),
    ContactPersist(PhysicsContactEvent),
    ContactEnd { a: EntityHandle, b: EntityHandle },
    BodyCreated { entity: EntityHandle },
    BodyDestroyed { entity: EntityHandle },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhysicsStepReport {
    pub fixed_tick: u64,
    pub dt: f32,
    pub substeps: u32,
    pub active_bodies: usize,
    pub static_bodies: usize,
    pub dynamic_bodies: usize,
    pub contacts: usize,
    pub commands_applied: usize,
    pub events: Vec<PhysicsEvent>,
}
