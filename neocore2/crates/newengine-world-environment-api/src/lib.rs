#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable service contract for the `engine.world.environment` gateway.
//!
//! Environment is world state. Render is one consumer of resolved DTO packets;
//! providers never receive native ECS ids, renderer handles or GPU history.

pub mod authored_profile;
mod celestial;
mod client;
mod consumers;
mod defaults;
mod frame;
mod intents;
mod objects;
mod primitives;
mod requests;
mod service;
mod spatial;
mod taxonomy;
mod weather;

pub use celestial::*;
pub use client::EnvironmentClient;
pub use consumers::*;
pub use frame::*;
pub use intents::*;
pub use objects::*;
pub use primitives::*;
pub use requests::*;
pub use service::*;
pub use spatial::*;
pub use taxonomy::*;
pub use weather::*;

#[cfg(test)]
mod tests;
