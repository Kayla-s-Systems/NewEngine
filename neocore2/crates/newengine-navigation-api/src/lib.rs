#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO contract for the `engine.navigation` gateway.

mod geometry;
mod requests;
mod service;

pub use geometry::*;
pub use requests::*;
pub use service::*;

#[cfg(test)]
mod tests;
