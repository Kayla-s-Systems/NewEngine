#![forbid(unsafe_op_in_unsafe_fn)]

pub mod debug_draw;
pub mod event_bridge;
pub mod fixed_step;
pub mod physics_residency;
pub mod physics_world_service;

pub use fixed_step::{FixedStepClock, FixedStepDrain};
pub use physics_residency::{PhysicsResidencyCommand, PhysicsResidencySet};
pub use physics_world_service::{step_physics_world, PhysicsWorldService, PhysicsWorldStepSettings};
