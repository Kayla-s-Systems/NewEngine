#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable service protocol for replaceable NewEngine physics backends.
//!
//! DTO packets contain only stable values and never expose ECS storage or
//! native backend handles across the service boundary.

mod backend;
mod codec;
mod collision;
mod commands;
mod frame;
mod protocol;
mod service;

pub use backend::*;
pub use codec::*;
pub use collision::*;
pub use commands::*;
pub use frame::*;
pub use protocol::*;
pub use service::*;

pub type PhysicsEntityKey = u64;
pub type PhysicsVec3 = [f32; 3];
pub type PhysicsQuat = [f32; 4];

#[cfg(test)]
mod tests;
