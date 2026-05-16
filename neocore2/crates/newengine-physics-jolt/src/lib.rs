#![forbid(unsafe_op_in_unsafe_fn)]

mod world;
pub mod backend;
pub mod body_map;
pub mod debug_adapter;
pub mod query_adapter;
pub mod shape_cooker;

pub use backend::JoltPhysicsBackend;
pub use world::{JoltInitDesc, PhysicsError, PhysicsWorld, WorldLimits};
