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
mod controllers;
mod schedule;
mod systems;
mod time;

pub use access::{AccessMask, Subsystem};
pub use commands::{Command, CommandBuffer};
pub use components::*;
pub use controllers::*;
pub use schedule::{default_schedule, SimSchedule, SimStage};
pub use time::SimFrame;

pub use systems::{
    sys_camera_rig_to_transform, sys_character_motor, sys_integrate_velocities, sys_orbit_camera,
    sys_scene_derived,
};
