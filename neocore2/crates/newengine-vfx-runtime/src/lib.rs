#![forbid(unsafe_op_in_unsafe_fn)]

//! Central VFX orchestration runtime.
//!
//! This is the runtime owner for semantic effects. Gameplay does not create muzzle
//! cones, impact sparks or decals directly; it submits effect intent here.

mod definitions;
mod runtime;

pub use definitions::*;
pub use runtime::*;

#[cfg(test)]
mod tests;
