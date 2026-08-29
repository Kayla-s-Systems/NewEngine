#![forbid(unsafe_op_in_unsafe_fn)]

use super::metadata::descriptor_gateway_capabilities;
use super::provider::gateway_provider_service_id;
use newengine_plugin_api::PluginDescriptor;
#[cfg(test)]
use newengine_service_api::EngineServiceKind;
use newengine_service_api::{
    engine_gateway_domain, engine_gateway_matches_service_kind, system_tag,
};
use std::path::Path;

#[path = "registry/active.rs"]
mod active;
#[path = "registry/descriptor.rs"]
mod descriptor;
#[path = "registry/facts.rs"]
mod facts;
#[path = "registry/route_model.rs"]
mod route_model;

pub(crate) use active::{
    descriptor_composition_candidates, descriptor_v2_composition_candidates,
    host_route_composition_candidates, ActiveGatewayRegistry,
};
pub(crate) use descriptor::{
    descriptor_engine_gateways, descriptor_engine_gateways_v2, descriptor_max_gateway_priority,
    descriptor_max_gateway_priority_v2,
};
#[cfg(test)]
pub(crate) use facts::GatewayOverrideMode;
pub(crate) use facts::{
    GatewayPolicyFact, GatewayProviderOrigin, GatewayProviderRouteFact, PluginDescriptorFact,
    RegisteredServiceFact,
};
pub(crate) use route_model::ActiveGatewayRoute;

#[cfg(test)]
mod tests;
