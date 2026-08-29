#![forbid(unsafe_op_in_unsafe_fn)]

//! Profile-owned FPS gameplay package.
//!
//! `newengine-engine-runtime` owns generic execution/UI boundaries and shared runtime
//! contracts. This crate owns reusable FPS mechanics and policy interpretation; authored game data is project-owned.

mod provider;

pub use newengine_gameplay_fps_api::{
    action as fps_action, FpsActionFrame, FpsDemoGoal, FpsDemoHazard, FpsDemoPickup, FpsDemoRules,
    FpsDemoState, FpsDemoTarget, FpsPlayerTuning,
};
pub use provider::FpsGameplayProvider;
