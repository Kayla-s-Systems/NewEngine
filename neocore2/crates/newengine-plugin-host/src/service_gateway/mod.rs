#![forbid(unsafe_op_in_unsafe_fn)]

mod metadata;
mod provider;
mod registry;
mod route;

pub use metadata::{descriptor_gateway_capabilities, EngineGatewayCapability};
pub(crate) use registry::{
    descriptor_engine_gateways, descriptor_engine_gateways_v2, descriptor_max_gateway_priority,
    descriptor_max_gateway_priority_v2, ActiveGatewayRegistry, ActiveGatewayRoute,
    GatewayPolicyFact, GatewayProviderOrigin, GatewayProviderRouteFact, PluginDescriptorFact,
    RegisteredServiceFact,
};

pub(crate) use route::provider_route_extends_gateway_parent;
