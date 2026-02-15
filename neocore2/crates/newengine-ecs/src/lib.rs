#![forbid(unsafe_op_in_unsafe_fn)]

mod entity;
mod component;
mod storage;
mod query;
mod world;
mod commands;

pub use entity::EntityId;

pub use component::Component;
pub use storage::{ErasedStorage, Storage};

pub use query::{Query, Query2, Query2A, Query2B, QueryMut};

pub use world::World;

pub use commands::{Command, Commands};
