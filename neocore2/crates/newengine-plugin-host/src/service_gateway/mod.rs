#![forbid(unsafe_op_in_unsafe_fn)]

mod metadata;
mod provider;
mod registry;

pub(crate) use metadata::descriptor_gateway_capabilities;
pub(crate) use provider::gateway_provider_service_id;
pub(crate) use registry::{
    descriptor_engine_gateways, descriptor_max_gateway_priority, ActiveGatewayRegistry,
    EngineOwnedGatewayFact, PluginDescriptorFact, RegisteredServiceFact,
};
