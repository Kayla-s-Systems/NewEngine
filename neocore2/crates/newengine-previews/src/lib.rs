#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-level dynamic preview rendering.
//!
//! This crate renders small offscreen previews (GPU render targets) for UI lists.
//! Apps are consumers: they request a preview and display the returned `UiTexId`.

mod primitive;

pub use primitive::{PrimitivePreviewService, PrimitivePreviewSize};
