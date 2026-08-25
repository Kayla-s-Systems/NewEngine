use super::facts::GatewayPolicyFact;
use super::route_model::route_matches_query;
use super::*;
use newengine_service_api::{
    parse_versioned_contract_id, CapabilityMatrix, CompositionCandidate, CompositionPlan,
    CompositionSolver, CompositionSolverInput,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct ActiveGatewayRegistry {
    routes: Vec<ActiveGatewayRoute>,
    /// Immutable provider-selection result. The registry is only a materialized
    /// route view over this plan and never performs independent selection.
    plan: CompositionPlan,
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
        Self::from_facts_with_policy_and_matrix(
            descriptors,
            services,
            gateway_provider_routes,
            policy_facts,
            CapabilityMatrix::default(),
        )
    }

    pub(crate) fn from_facts_with_policy_and_matrix(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        gateway_provider_routes: &[GatewayProviderRouteFact],
        policy_facts: &[GatewayPolicyFact],
        capability_matrix: CapabilityMatrix,
    ) -> Self {
        let mut routes = Vec::new();
        let mut skipped_unregistered = 0usize;

        for descriptor_fact in descriptors {
            let gateways = descriptor_fact
                .descriptor_v2
                .as_ref()
                .map(crate::service_gateway::metadata::descriptor_gateway_capabilities_v2)
                .unwrap_or_else(|| descriptor_gateway_capabilities(&descriptor_fact.descriptor));
            for gateway in gateways {
                let provider_service_id =
                    if let Some(descriptor_v2) = descriptor_fact.descriptor_v2.as_ref() {
                        crate::service_gateway::provider::gateway_provider_service_id_v2(
                            descriptor_v2,
                            &gateway,
                        )
                    } else {
                        gateway_provider_service_id(&descriptor_fact.descriptor, &gateway)
                    };
                let Some(provider_service_id) = provider_service_id else {
                    continue;
                };

                let registered = services.iter().any(|service| {
                    service.service_id == provider_service_id
                        && service.owner_plugin_id.as_deref()
                            == Some(descriptor_fact.plugin_id.as_str())
                });
                if !registered {
                    skipped_unregistered += 1;
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

        let candidates = routes
            .iter()
            .map(|route| {
                let mut candidate = CompositionCandidate::new(
                    route.gateway_id.clone(),
                    route.selection_key.clone(),
                    route.provider_owner_id.clone(),
                    route.backend_priority,
                    route.origin.origin_bias(),
                    route.selection_bonus,
                )
                .with_capability(route.backend_capability_id.clone())
                .with_tags(route.system_tags.clone());
                if let Some((contract_id, version)) = route
                    .provider_abi
                    .as_deref()
                    .and_then(parse_versioned_contract_id)
                {
                    candidate = candidate.with_contract(contract_id, version);
                }
                candidate
            })
            .collect();

        let plan = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates,
            capability_matrix,
        });

        // Diagnostics-only ordering. The plan above remains the only authority.
        routes.sort_by(|a, b| {
            a.gateway_id
                .cmp(&b.gateway_id)
                .then_with(|| b.active_score.cmp(&a.active_score))
                .then_with(|| a.selection_key.cmp(&b.selection_key))
        });

        let registry = Self { routes, plan };
        newengine_ulog_api::ulog::debug!(
            "gateways: composition plan rebuilt descriptors={} services={} host_routes={} policy_facts={} routes={} gateways={} unsatisfied={} skipped_unregistered={}",
            descriptors.len(),
            services.len(),
            gateway_provider_routes.len(),
            policy_facts.len(),
            registry.routes.len(),
            registry.plan.gateway_ids().len(),
            registry.plan.unsatisfied().len(),
            skipped_unregistered
        );
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
                active_route
                    .as_ref()
                    .is_none_or(|active| route.selection_key != active.selection_key)
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
        self.plan.gateway_ids()
    }

    pub(crate) fn resolve_route(&self, gateway_id: &str) -> Option<&ActiveGatewayRoute> {
        let selected = self.plan.selected(gateway_id)?;
        self.routes
            .iter()
            .find(|route| route.selection_key == selected.candidate_id)
    }

    pub(crate) fn validate_required_requirements(&self) -> Result<(), String> {
        self.plan.validate_required()
    }

    pub(crate) fn has_gateway_capability(&self, gateway_id: &str, capability_id: &str) -> bool {
        self.routes.iter().any(|route| {
            route_matches_query(route, gateway_id) && route.backend_capability_id == capability_id
        })
    }
}
