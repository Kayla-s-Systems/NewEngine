#![forbid(unsafe_op_in_unsafe_fn)]

//! NewEngine Materials
//!
//! Design goals:
//! - Deterministic ids and stable iteration order.
//! - Clean separation: public API (data contracts) vs runtime registry implementation.
//! - Extensible model: builtins are just providers; plugins can register more.

pub mod api;
pub mod builtin;
pub mod core;

#[cfg(feature = "serde")]
pub mod serde;

mod errors;

pub use crate::api::{MaterialDescriptor, MaterialFlags, MaterialId, MaterialRef};
pub use crate::core::MaterialRegistry;
pub use crate::errors::{MaterialError, MaterialResult};
