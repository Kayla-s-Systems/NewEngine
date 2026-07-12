#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable raw-input service contract, canonical device identities and provider snapshots.

mod contracts;
mod snapshot;

pub mod engine_default_keybind;
pub mod gamepad_axis;
pub mod gamepad_button;
pub mod key_code;
pub mod key_identity;
pub mod mouse_button;

pub use contracts::*;
pub use snapshot::{InputGamepadSnapshot, InputStateSnapshot};

#[cfg(test)]
mod tests;
