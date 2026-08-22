use super::*;
use super::facts::GatewayPolicyFact;
use super::route_model::route_matches_query;

#[derive(Debug, Clone, Default)]
pub(crate) struct ActiveGatewayRegistry {
    routes: Vec<ActiveGatewayRoute>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GatewayRouteDiagnostics {
    pub(crate) gateway_id: String,
    pub(crate) active_route: Option<ActiveGatewayRoute>,
    pub(crate) shadowed_routes: Vec<ActiveGatewayRoute>,
}

impl ActiveGatewayRegistry {
    #[cfg(test)]
    pub(crate) fn from_facts(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        gateway_provider_routes: &[GatewayProviderRouteFact],
    ) -> Self {
        Self::from_facts_with_policy(descriptors, services, gateway_provider_routes, &[])
    }

    pub(crate) fn from_facts_with_policy(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        gateway_provider_routes: &[GatewayProviderRouteFact],
        policy_facts: &[GatewayPolicyFact],
    ) -> Self {
        let mut routes = Vec::new();
        let mut skipped_unregistered = 0usize;

        for descriptor_fact in descriptors {
            for gateway in descriptor_gateway_capabilities(&descriptor_fact.descriptor) {
                let Some(provider_service_id) =
                    gateway_provider_service_id(&descriptor_fact.descriptor, &gateway)
                else {
                    continue;
                };

                let registered = services.iter().any(|service| {
                    service.service_id == provider_service_id
                        && service.owner_plugin_id.as_deref()
                            == Some(descriptor_fact.plugin_id.as_str())
                });
                if !registered {
                    skipped_unregistered += 1;
                    newengine_ulog_api::ulog::trace!(
                        "gateways: plugin route skipped because service is not registered plugin='{}' gateway='{}' service='{}' capability='{}'",
                        descriptor_fact.plugin_id,
                        gateway.gateway_id,
                        provider_service_id,
                        gateway.backend_capability_id
                    );
                    continue;
                }

                let policy = policy_facts
                    .iter()
                    .find(|policy| policy.gateway_id == gateway.gateway_id);
                if let Some(route) = ActiveGatewayRoute::new(
                    gateway.gateway_id,
                    gateway.service_kind,
                    provider_service_id,
                    gateway.provider_route_id,
                    gateway.provider_abi,
                    descriptor_fact.plugin_id.clone(),
                    gateway.backend_capability_id,
                    gateway.backend_priority,
                    descriptor_fact.origin,
                    gateway.system_tags,
                    policy,
                ) {
                    routes.push(route);
                }
            }
        }

        for gateway in gateway_provider_routes {
            let registered = services.iter().any(|service| {
                service.service_id == gateway.provider_service_id
                    && service.owner_plugin_id.is_none()
            });
            if !registered {
                skipped_unregistered += 1;
                newengine_ulog_api::ulog::trace!(
                    "gateways: engine-runtime route skipped because service is not registered gateway='{}' service='{}' owner='{}'",
                    gateway.gateway_id,
                    gateway.provider_service_id,
                    gateway.provider_owner_id
                );
                continue;
            }

            let policy = policy_facts
                .iter()
                .find(|policy| policy.gateway_id == gateway.gateway_id);
            if let Some(route) = ActiveGatewayRoute::new(
                gateway.gateway_id.clone(),
                gateway.service_kind.clone(),
                gateway.provider_service_id.clone(),
                Some(gateway.provider_route_id.clone()),
                gateway.provider_abi.clone(),
                gateway.provider_owner_id.clone(),
                gateway.backend_capability_id.clone(),
                gateway.backend_priority,
                gateway.origin,
                gateway.system_tags.clone(),
                policy,
            ) {
                routes.push(route);
            }
        }

        routes.sort_by(|a, b| {
            a.gateway_id
                .cmp(&b.gateway_id)
                .then_with(|| b.active_score.cmp(&a.active_score))
                .then_with(|| b.backend_priority.cmp(&a.backend_priority))
                .then_with(|| b.origin.origin_bias().cmp(&a.origin.origin_bias()))
                .then_with(|| a.service_kind.cmp(&b.service_kind))
                .then_with(|| a.provider_service_id.cmp(&b.provider_service_id))
                .then_with(|| a.provider_owner_id.cmp(&b.provider_owner_id))
        });

        let registry = Self { routes };
        newengine_ulog_api::ulog::debug!(
            "gateways: registry rebuilt descriptors={} services={} host_routes={} policy_facts={} routes={} skipped_unregistered={}",
            descriptors.len(),
            services.len(),
            gateway_provider_routes.len(),
            policy_facts.len(),
            registry.routes.len(),
            skipped_unregistered
        );
        for gateway_id in registry.gateway_ids() {
            let diagnostics = registry.route_diagnostics(&gateway_id);
            if let Some(route) = diagnostics.active_route.as_ref() {
                newengine_ulog_api::ulog::trace!(
                    "gateways: active route gateway='{}' service='{}' provider_route='{}' owner='{}' kind='{}' origin='{}' mode='{}' prio={} score={} tags='{}' shadowed={}",
                    diagnostics.gateway_id,
                    route.provider_service_id,
                    route.provider_route_id.as_deref().unwrap_or("<provider-route-unset>"),
                    route.provider_owner_id,
                    route.service_kind,
                    route.origin.as_str(),
                    route.override_mode.as_str(),
                    route.backend_priority,
                    route.active_score,
                    route.system_tags.join(","),
                    diagnostics.shadowed_routes.len()
                );
            }
            for shadowed in diagnostics.shadowed_routes.iter() {
                newengine_ulog_api::ulog::trace!(
                    "gateways: shadowed route gateway='{}' service='{}' provider_route='{}' owner='{}' kind='{}' origin='{}' mode='{}' prio={} score={} tags='{}'",
                    diagnostics.gateway_id,
                    shadowed.provider_service_id,
                    shadowed.provider_route_id.as_deref().unwrap_or("<provider-route-unset>"),
                    shadowed.provider_owner_id,
                    shadowed.service_kind,
                    shadowed.origin.as_str(),
                    shadowed.override_mode.as_str(),
                    shadowed.backend_priority,
                    shadowed.active_score,
                    shadowed.system_tags.join(",")
                );
            }
        }

        registry
    }

    pub(crate) fn routes(&self) -> &[ActiveGatewayRoute] {
        &self.routes
    }

    pub(crate) fn route_diagnostics(&self, gateway_id: &str) -> GatewayRouteDiagnostics {
        let active_route = self.resolve_route(gateway_id).cloned();
        let shadowed_routes = self
            .routes
            .iter()
            .filter(|route| route_matches_query(route, gateway_id))
            .filter(|route| {
                active_route.as_ref().is_none_or(|active| {
                    route.provider_service_id != active.provider_service_id
                        || route.provider_route_id != active.provider_route_id
                        || route.provider_owner_id != active.provider_owner_id
                })
            })
            .cloned()
            .collect::<Vec<_>>();

        GatewayRouteDiagnostics {
            gateway_id: gateway_id.to_owned(),
            active_route,
            shadowed_routes,
        }
    }

    pub(crate) fn gateway_ids(&self) -> Vec<String> {
        let mut out = self
            .routes
            .iter()
            .map(|route| route.gateway_id.clone())
            .collect::<Vec<_>>();
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn resolve_route(&self, gateway_id: &str) -> Option<&ActiveGatewayRoute> {
        self.routes
            .iter()
            .filter(|route| route_matches_query(route, gateway_id))
            .max_by(|a, b| {
                a.active_score
                    .cmp(&b.active_score)
                    .then_with(|| a.backend_priority.cmp(&b.backend_priority))
                    .then_with(|| a.origin.origin_bias().cmp(&b.origin.origin_bias()))
                    .then_with(|| b.provider_service_id.cmp(&a.provider_service_id))
                    .then_with(|| b.provider_owner_id.cmp(&a.provider_owner_id))
            })
    }

    pub(crate) fn has_gateway_capability(&self, gateway_id: &str, capability_id: &str) -> bool {
        self.routes.iter().any(|route| {
            route_matches_query(route, gateway_id) && route.backend_capability_id == capability_id
        })
    }
}
