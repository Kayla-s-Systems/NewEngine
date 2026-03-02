#![forbid(unsafe_op_in_unsafe_fn)]

//! Game-ready simulation layer on top of `newengine-ecs`.
//!
//! This crate intentionally stays renderer/editor agnostic.
//!
//! # Parallel execution
//!
//! Enable the `parallel` feature to execute non-conflicting systems in parallel.
//! Systems declare coarse-grained access via [`AccessMask`]. The executor groups
//! systems into deterministic batches (stable by `(order, seq)`), runs each batch
//! in parallel, and then commits each system's [`CommandBuffer`] in deterministic
//! order.

mod access;
mod commands;
mod components;
mod controller_ctx;
mod controllers;
mod intent;
mod schedule;
mod systems;
mod time;
mod transform_cmd;

pub use access::{AccessMask, Subsystem};
pub use commands::{Command, CommandBuffer};
// Re-export simulation components/controllers at crate root for ergonomic use by editor/runtime.
// Keep explicit re-exports to avoid accidental API disappearance when modules evolve.
pub use components::{
    AngularVelocity, CameraInputComp, CameraRigComp, CharacterMotor, FollowTargetCameraController,
    FollowTargetCameraMotor, MotorInput, OrbitCameraMotor, Velocity,
};
pub use controller_ctx::{ControllerCtx, EntityControllerV1};
pub use controllers::{
    follow_params_from_pose, run_character_motor_controller, run_follow_camera_controller,
    run_orbit_camera_controller, step_character_motor, step_follow_camera, CharacterMotorStep,
    FollowCameraStep,
};
pub use intent::{
    ControllerIntentQueue, Intent, IntentBuffer, IntentCommandBufferExt, IntentSink,
};

// Also keep glob re-exports for backwards compatibility inside the workspace.
pub use components::*;
pub use controllers::*;
pub use schedule::{default_schedule, SimSchedule, SimStage};
pub use time::SimFrame;

pub use transform_cmd::TransformCommandBufferExt;

pub use systems::{
    sys_apply_controller_intents, sys_camera_follow, sys_camera_rig_to_transform,
    sys_character_motor, sys_integrate_velocities, sys_orbit_camera,
};
