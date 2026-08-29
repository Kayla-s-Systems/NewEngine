use crate::{PhysicsCommand, PhysicsEvent};

#[derive(Clone, Debug, PartialEq)]
pub enum PhysicsReplayEvent {
    Command(PhysicsCommand),
    Event(PhysicsEvent),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhysicsReplayFrame {
    pub fixed_tick: u64,
    pub dt: f32,
    pub events: Vec<PhysicsReplayEvent>,
}
