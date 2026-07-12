#![forbid(unsafe_op_in_unsafe_fn)]

//! Concrete entity-domain value types.
//!
//! Entity identity remains owned by `newengine-entity-api`; this crate provides
//! engine-side classification and non-authoritative authoring metadata.

mod kind;
mod metadata;
mod name;

pub use kind::EntityKind;
pub use metadata::EntityMeta;
pub use name::EntityName;

// Entity identity is defined in the stable API crate so higher-level API/contract
// crates can name entities without pulling the concrete ECS/world implementation.
pub use newengine_entity_api::EntityId;
