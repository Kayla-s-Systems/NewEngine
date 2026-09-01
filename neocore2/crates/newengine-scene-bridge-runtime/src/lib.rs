#![forbid(unsafe_op_in_unsafe_fn)]

//! Scene/world bridge, editor scene projection and bootstrap semantics outside composition root.

pub mod editor_viewport_adapter;
mod scene_bootstrap;
pub mod scene_bridge;
pub mod world_authoring;

pub use scene_bridge::*;
