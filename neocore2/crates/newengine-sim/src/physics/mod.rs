#![forbid(unsafe_op_in_unsafe_fn)]

mod types;
mod bootstrap;
mod bake;
mod cleanup;
mod kinematic;
mod step;
mod sync;
mod jolt;

pub use types::{
    Collider, PhysicsBody, PhysicsCtx, PhysicsDebugStats, PhysicsInitDesc, PhysicsPose,
    PhysicsSettings, PhysicsStepState, RigidBody, RigidBodyKind,
};

pub use bootstrap::{
    physics_bootstrap, physics_bootstrap_default, physics_debug_stats,
    physics_set_interpolation_alpha,
};

pub use bake::physics_bake_bodies;
pub use cleanup::physics_cleanup_bodies;
pub use step::physics_step_jolt;
pub use sync::physics_sync_transforms;
