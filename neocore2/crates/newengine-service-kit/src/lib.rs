#![forbid(unsafe_op_in_unsafe_fn)]

pub mod engine_owned_gateway;
pub mod json_service;
pub mod method_router;
pub mod provider_metadata;

pub use engine_owned_gateway::{
    register_engine_owned_gateway_service, register_engine_owned_gateway_service_best_effort,
    EngineOwnedGatewayDecl,
};
pub use json_service::{decode_json_payload, empty_payload_json, ok_empty_blob, ok_json, payload_json};
pub use method_router::JsonServiceRouter;
pub use provider_metadata::{engine_owned_service_description, EngineOwnedServiceDescription};
