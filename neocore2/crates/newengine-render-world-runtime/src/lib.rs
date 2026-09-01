#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::too_many_arguments)]

//! Render-world implementation runtime outside the engine composition root.
//! Owns render extraction, GPU resource policy, world rendering and frame submission.

pub mod render_controller;

pub use render_controller::RuntimeRenderController;
