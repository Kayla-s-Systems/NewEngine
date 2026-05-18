#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::PluginDescriptor;
use newengine_service_api::EngineServiceKind;

use super::metadata::descriptor_gateway_capabilities;
use super::provider::gateway_provider_service_id;

#[derive(Debug, Clone)]
pub(crate) struct RegisteredServiceFact {
    pub(crate) service_id: String,
    pub(crate) owner_plugin_id: Option<String>,
}

impl RegisteredServiceFact {
    #[inline]
    pub(crate) fn new(service_id: String, owner_plugin_id: Option<String>) -> Self {
        Self { service_id, owner_plugin_id }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PluginDescriptorFact {
    pub(crate) plugin_id: String,
    pub(crate) descriptor: PluginDescriptor,
}

impl PluginDescriptorFact {
    #[inline]
    pub(crate) fn new(plugin_id: String, descriptor: PluginDescriptor) -> Self {
        Self { plugin_id, descriptor }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EngineOwnedGatewayFact {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: EngineServiceKind,
    pub(crate) provider_service_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
}

impl EngineOwnedGatewayFact {
    #[inline]
    pub(crate) fn new(
        gateway_id: String,
        service_kind: EngineServiceKind,
        provider_service_id: String,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
    ) -> Self {
        Self {
            gateway_id,
            service_kind,
            provider_service_id,
            provider_owner_id,
            backend_capability_id,
            backend_priority,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GatewayRouteOrigin {
    Plugin,
    EngineOwned,
}

impl GatewayRouteOrigin {
    #[inline]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Plugin => "plugin",
            Self::EngineOwned => "engine-owned",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveGatewayRoute {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: EngineServiceKind,
    pub(crate) provider_service_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) origin: GatewayRouteOrigin,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ActiveGatewayRegistry {
    routes: Vec<ActiveGatewayRoute>,
}

impl ActiveGatewayRegistry {
    pub(crate) fn from_facts(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        engine_owned_gateways: &[EngineOwnedGatewayFact],
    ) -> Self {
        let mut routes = Vec::new();

        for descriptor_fact in descriptors {
            for gateway in descriptor_gateway_capabilities(&descriptor_fact.descriptor) {
                let Some(provider_service_id) =
                    gateway_provider_service_id(&descriptor_fact.descriptor, &gateway)
                else {
                    continue;
                };

                let registered = services.iter().any(|service| {
                    service.service_id == provider_service_id
                        && service.owner_plugin_id.as_deref() == Some(descriptor_fact.plugin_id.as_str())
                });
                if !registered {
                    continue;
                }

                routes.push(ActiveGatewayRoute {
                    gateway_id: gateway.gateway_id,
                    service_kind: gateway.service_kind,
                    provider_service_id,
                    provider_owner_id: descriptor_fact.plugin_id.clone(),
                    backend_capability_id: gateway.backend_capability_id,
                    backend_priority: gateway.backend_priority,
                    origin: GatewayRouteOrigin::Plugin,
                });
            }
        }

        for gateway in engine_owned_gateways {
            let registered = services.iter().any(|service| {
                service.service_id == gateway.provider_service_id && service.owner_plugin_id.is_none()
            });
            if !registered {
                continue;
            }

            routes.push(ActiveGatewayRoute {
                gateway_id: gateway.gateway_id.clone(),
                service_kind: gateway.service_kind,
                provider_service_id: gateway.provider_service_id.clone(),
                provider_owner_id: gateway.provider_owner_id.clone(),
                backend_capability_id: gateway.backend_capability_id.clone(),
                backend_priority: gateway.backend_priority,
                origin: GatewayRouteOrigin::EngineOwned,
            });
        }

        routes.sort_by(|a, b| {
            a.gateway_id
                .cmp(&b.gateway_id)
                .then_with(|| a.service_kind.as_str().cmp(&b.service_kind.as_str()))
                .then_with(|| b.backend_priority.cmp(&a.backend_priority))
                .then_with(|| a.origin.as_str().cmp(&b.origin.as_str()))
                .then_with(|| a.provider_service_id.cmp(&b.provider_service_id))
                .then_with(|| a.provider_owner_id.cmp(&b.provider_owner_id))
        });

        Self { routes }
    }

    pub(crate) fn routes(&self) -> &[ActiveGatewayRoute] {
        &self.routes
    }

    pub(crate) fn gateway_ids(&self) -> Vec<String> {
        let mut out = self.routes.iter().map(|route| route.gateway_id.clone()).collect::<Vec<_>>();
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn resolve_gateway(&self, gateway_id: &str) -> Option<String> {
        self.resolve_route(gateway_id)
            .map(|route| route.provider_service_id.clone())
    }

    pub(crate) fn resolve_route(&self, gateway_id: &str) -> Option<&ActiveGatewayRoute> {
        self.routes
            .iter()
            .filter(|route| route.gateway_id == gateway_id)
            .max_by(|a, b| {
                a.backend_priority
                    .cmp(&b.backend_priority)
                    .then_with(|| a.service_kind.as_str().cmp(&b.service_kind.as_str()))
                    .then_with(|| b.origin.as_str().cmp(&a.origin.as_str()))
                    .then_with(|| b.provider_service_id.cmp(&a.provider_service_id))
                    .then_with(|| b.provider_owner_id.cmp(&a.provider_owner_id))
            })
    }

    pub(crate) fn has_gateway_capability(&self, gateway_id: &str, capability_id: &str) -> bool {
        self.routes.iter().any(|route| {
            route.gateway_id == gateway_id && route.backend_capability_id == capability_id
        })
    }
}

pub(crate) fn descriptor_engine_gateways(descriptor: &PluginDescriptor) -> Vec<String> {
    let mut out: Vec<String> = descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter_map(|gateway| gateway_provider_service_id(descriptor, &gateway).map(|_| gateway.gateway_id))
        .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn descriptor_max_gateway_priority(descriptor: &PluginDescriptor) -> i32 {
    descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter(|gateway| gateway_provider_service_id(descriptor, gateway).is_some())
        .map(|gateway| gateway.backend_priority)
        .max()
        .unwrap_or(0)
}
