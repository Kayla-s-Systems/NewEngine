#![forbid(unsafe_op_in_unsafe_fn)]

pub mod gateway_provider_route;
pub mod json_service;
pub mod method_router;
pub mod provider_metadata;

pub use gateway_provider_route::{
    register_engine_gateway_provider_service, register_engine_gateway_provider_service_best_effort,
    register_engine_gateway_provider_service_dynamic,
    register_engine_gateway_provider_service_dynamic_best_effort,
    register_null_engine_gateway_provider_service_dynamic,
    register_null_engine_gateway_provider_service_dynamic_best_effort, EngineGatewayProviderDecl,
    EngineGatewayProviderDeclDynamic, NullEngineGatewayProviderDeclDynamic,
};
pub use json_service::{
    decode_json_payload, empty_payload_json, ok_empty_blob, ok_json, payload_json,
};
pub use method_router::JsonServiceRouter;
pub use provider_metadata::{
    engine_gateway_provider_service_description, EngineGatewayProviderServiceDescription,
};
