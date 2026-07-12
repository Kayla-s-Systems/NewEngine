#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO contract for the `engine.tags` gateway.
//!
//! Tags are the common gameplay vocabulary consumed by AI, tasks, animation,
//! interaction, scripting and debug tools. They are data declarations, not
//! hardcoded gameplay branches.

mod descriptor;
mod requests;
mod service;
mod snapshot;

pub use descriptor::*;
pub use requests::*;
pub use service::*;
pub use snapshot::*;

#[cfg(test)]
mod tests;
