#![forbid(unsafe_op_in_unsafe_fn)]

//! Standard profile-owned render feature pack.
//!
//! This crate is not a renderer backend and does not depend on a concrete
//! runtime controller. It implements provider traits from
//! `newengine-render-feature-api`; the active profile composes these providers
//! into whatever runtime owns the render feature registries.

mod constants;
mod draw;
mod lighting;
mod pack;

pub use constants::*;
pub use pack::StandardRenderFeaturePack;

#[cfg(test)]
mod tests;
