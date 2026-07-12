#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.entity` gateway runtime service.
//!
//! Entity lifecycle calls are exposed through stable DTOs from
//! `newengine-entity-api` and operate against the shared runtime scene bridge.

mod operations;
mod payload;
mod registration;
mod router;
mod service;

pub use registration::register_entity_gateway_best_effort;
pub use router::entity_gateway_service;
pub use service::{EngineEntityGatewayService, ENTITY_GATEWAY_OWNER};
