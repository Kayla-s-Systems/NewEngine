#![forbid(unsafe_op_in_unsafe_fn)]

mod metadata;
mod provider;
mod registry;
mod route;

pub use metadata::{descriptor_gateway_capabilities, EngineGatewayCapability};
pub(crate) use provider::gateway_provider_service_id;
pub(crate) use registry::{
    descriptor_engine_gateways, descriptor_max_gateway_priority, ActiveGatewayRegistry,
    ActiveGatewayRoute, GatewayProviderOrigin, GatewayProviderRouteFact, PluginDescriptorFact,
    RegisteredServiceFact,
};

pub(crate) use route::provider_route_extends_gateway_parent;
