#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use newengine_plugin_api::PluginDescriptor;
#[cfg(test)]
use newengine_service_api::EngineServiceKind;
use newengine_service_api::{engine_gateway_domain, engine_gateway_matches_service_kind, system_tag};
use super::metadata::descriptor_gateway_capabilities;
use super::provider::gateway_provider_service_id;

#[path = "registry/facts.rs"]
mod facts;
#[path = "registry/route_model.rs"]
mod route_model;
#[path = "registry/active.rs"]
mod active;
#[path = "registry/descriptor.rs"]
mod descriptor;

pub(crate) use active::ActiveGatewayRegistry;
pub(crate) use descriptor::{descriptor_engine_gateways, descriptor_max_gateway_priority};
pub(crate) use facts::{GatewayProviderOrigin, GatewayProviderRouteFact, PluginDescriptorFact, RegisteredServiceFact};
#[cfg(test)]
pub(crate) use facts::{GatewayOverrideMode, GatewayPolicyFact};
pub(crate) use route_model::ActiveGatewayRoute;

#[cfg(test)]
mod tests;
