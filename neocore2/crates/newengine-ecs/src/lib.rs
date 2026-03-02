#![forbid(unsafe_op_in_unsafe_fn)]

//! # newengine-ecs
//!
//! Foundation-first, deterministic ECS for NewEngine.
//!
//! ## Core properties
//! - **Deterministic entity identity** via generational keys (`EntityId`).
//! - **Type-safe component storages** with type erasure only at the `World` boundary.
//! - **Zero-allocation iteration** for queries (thin iterator wrappers).
//! - **Explicit structural changes** via a two-phase command buffer (`Commands`).
//! - **Conservative change tracking** (`added_tick` / `changed_tick`) driven by a monotonic `World::tick`.
//!
//! ## Threading model
//! `World` is `Send + Sync` as long as components and resources are `Send + Sync`.
//! This enables scene bridges and editor tooling to share the world safely.
//!
//! ## Determinism notes
//! - The ECS itself does not rely on hash-map iteration order for gameplay results.
//! - Use explicit schedules (`Schedule`) and system ordering to keep simulation deterministic.
//! - `TypeId` is an internal implementation detail and must not be used as a persistent identifier.

mod component;
mod storage;
mod query;
mod world;
mod commands;
mod events;
mod schedule;

pub use newengine_entity::EntityId;

pub use component::Component;
pub use storage::{ErasedStorage, Storage};

pub use query::{Query, Query2, Query2A, Query2B, QueryMut, QueryMutTracked};

pub use world::World;

pub use commands::{Commands, EntityToken};

pub use events::Events;

pub use schedule::{FrameCtx, Schedule, Stage, System};
