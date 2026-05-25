#![forbid(unsafe_op_in_unsafe_fn)]

//! UI input DTO compatibility re-export.
//!
//! The canonical type is owned by `newengine-ui-api` so reusable runtime code
//! can communicate with `engine.ui` without importing this concrete UI crate.

pub use newengine_ui_api::{keys, UiInputFrame};
