#![forbid(unsafe_op_in_unsafe_fn)]

//! Backend-neutral NewEngine physics contracts.
//!
//! Gameplay, scene streaming and AssetManager must depend on these contracts,
//! never on a backend-native handle such as JPH::BodyID. Backends own their
//! handle maps internally.

pub mod body;
pub mod events;
pub mod material;
pub mod query;
pub mod replay;
pub mod shape;
pub mod world;

pub use body::{CharacterControllerDesc, PhysicsBodyDesc, PhysicsBodyKind, PhysicsBodyRuntimeFlags, PhysicsHandle};
pub use events::{PhysicsContactEvent, PhysicsEvent, PhysicsStepReport};
pub use material::PhysicsMaterialDesc;
pub use query::{PhysicsQuery, PhysicsQueryHit, PhysicsQueryKind};
pub use replay::{PhysicsReplayEvent, PhysicsReplayFrame};
pub use shape::CollisionShapeDesc;
pub use world::{PhysicsBackendKind, PhysicsCommand, PhysicsCommandKind, PhysicsWorldDesc};
