use super::facts::GatewayPolicyFact;
use super::route_model::route_matches_query;
use super::*;
#[cfg(test)]
use newengine_service_api::CompositionCandidateDisposition;
use newengine_service_api::{
    CapabilityMatrix, CompositionContractResolution, CompositionContractResolutionSubject,
    CompositionExplanationGraph, CompositionPlan, CompositionSolver, CompositionSolverInput,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct ActiveGatewayRegistry {
    routes: Vec<ActiveGatewayRoute>,
    /// Immutable provider-selection result. The registry is only a materialized
    /// route view over this plan and never performs independent selection.
    plan: CompositionPlan,
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(crate) struct GatewayRouteDiagnostics {
    pub(crate) gateway_id: String,
    pub(crate) active_route: Option<ActiveGatewayRoute>,
    pub(crate) shadowed_routes: Vec<ActiveGatewayRoute>,
}

pub(crate) fn descriptor_composition_candidates(
    descriptor: &newengine_plugin_api::PluginDescriptor,
    origin: GatewayProviderOrigin,
    policy_facts: &[GatewayPolicyFact],
) -> Vec<newengine_service_api::CompositionCandidate> {
    crate::service_gateway::metadata::descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter_map(|gateway| {
            let provider_service_id = gateway_provider_service_id(descriptor, &gateway)?;
            let policy = policy_facts
                .iter()
                .find(|policy| policy.gateway_id == gateway.gateway_id);
            ActiveGatewayRoute::new(
                gateway.gateway_id,
                gateway.service_kind,
                provider_service_id,
                gateway.provider_route_id,
                gateway.provider_abi,
                descriptor.id.to_string(),
                gateway.backend_capability_id,
                Some(gateway.capability_version),
                gateway.contract_id,
                gateway.contract_version,
                gateway.backend_priority,
                origin,
                gateway.system_tags,
                policy,
            )
        })
        .map(|route| route.composition_candidate())
        .collect()
}

pub(crate) fn descriptor_v2_composition_candidates(
    descriptor: &newengine_plugin_api::PluginDescriptorV2,
    origin: GatewayProviderOrigin,
    policy_facts: &[GatewayPolicyFact],
) -> Vec<newengine_service_api::CompositionCandidate> {
    crate::service_gateway::metadata::descriptor_gateway_capabilities_v2(descriptor)
        .into_iter()
        .filter_map(|gateway| {
            let provider_service_id =
                crate::service_gateway::provider::gateway_provider_service_id_v2(
                    descriptor, &gateway,
                )?;
            let policy = policy_facts
                .iter()
                .find(|policy| policy.gateway_id == gateway.gateway_id);
            ActiveGatewayRoute::new(
                gateway.gateway_id,
                gateway.service_kind,
                provider_service_id,
                gateway.provider_route_id,
                gateway.provider_abi,
                descriptor.id.to_string(),
                gateway.backend_capability_id,
                Some(gateway.capability_version),
                gateway.contract_id,
                gateway.contract_version,
                gateway.backend_priority,
                origin,
                gateway.system_tags,
                policy,
            )
        })
        .map(|route| route.composition_candidate())
        .collect()
}

pub(crate) fn host_route_composition_candidates(
    services: &[RegisteredServiceFact],
    gateway_provider_routes: &[GatewayProviderRouteFact],
    policy_facts: &[GatewayPolicyFact],
) -> Vec<newengine_service_api::CompositionCandidate> {
    gateway_provider_routes
        .iter()
        .filter(|gateway| {
            services.iter().any(|service| {
                service.service_id == gateway.provider_service_id
                    && service.owner_plugin_id.is_none()
            })
        })
        .filter_map(|gateway| {
            let policy = policy_facts
                .iter()
                .find(|policy| policy.gateway_id == gateway.gateway_id);
            ActiveGatewayRoute::new(
                gateway.gateway_id.clone(),
                gateway.service_kind.clone(),
                gateway.provider_service_id.clone(),
                Some(gateway.provider_route_id.clone()),
                gateway.provider_abi.clone(),
                gateway.provider_owner_id.clone(),
                gateway.backend_capability_id.clone(),
                None,
                None,
                None,
                gateway.backend_priority,
                gateway.origin,
                gateway.system_tags.clone(),
                policy,
            )
        })
        .map(|route| route.composition_candidate())
        .collect()
}
struct RuntimeContractResolutionInput<'a> {
    subject: CompositionContractResolutionSubject,
    gateway_id: &'a str,
    candidate_id: Option<String>,
    capability_id: String,
    reference: &'a str,
    min_version: u32,
    max_version: Option<u32>,
}

fn runtime_contract_resolution(
    input: RuntimeContractResolutionInput<'_>,
    entry: &newengine_runtime_contract_catalog::RuntimeContractEntry,
) -> CompositionContractResolution {
    CompositionContractResolution {
        subject: input.subject,
        gateway_id: input.gateway_id.to_owned(),
        candidate_id: input.candidate_id,
        capability_id: input.capability_id,
        reference: input.reference.to_owned(),
        canonical_id: entry.spec.key.clone(),
        min_version: input.min_version,
        max_version: input.max_version,
        authority: match entry.authority {
            newengine_runtime_contract_catalog::RuntimeContractAuthority::Engine => "engine",
            newengine_runtime_contract_catalog::RuntimeContractAuthority::Plugin => "plugin",
        }
        .to_owned(),
        owner: entry.spec.owner.clone(),
    }
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

    #[cfg(test)]
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

    #[cfg(test)]
    pub(crate) fn from_facts_with_policy_and_matrix(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        gateway_provider_routes: &[GatewayProviderRouteFact],
        policy_facts: &[GatewayPolicyFact],
        capability_matrix: CapabilityMatrix,
    ) -> Self {
        Self::from_facts_with_policy_matrix_and_plan(
            descriptors,
            services,
            gateway_provider_routes,
            policy_facts,
            capability_matrix,
            None,
        )
    }

    pub(crate) fn from_facts_with_policy_matrix_and_plan(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        gateway_provider_routes: &[GatewayProviderRouteFact],
        policy_facts: &[GatewayPolicyFact],
        capability_matrix: CapabilityMatrix,
        frozen_plan: Option<&CompositionPlan>,
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
                    Some(gateway.capability_version),
                    gateway.contract_id,
                    gateway.contract_version,
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
                None,
                None,
                None,
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
            .map(ActiveGatewayRoute::composition_candidate)
            .collect();

        let plan = frozen_plan.cloned().unwrap_or_else(|| {
            CompositionSolver::resolve_input(CompositionSolverInput {
                candidates,
                capability_matrix,
            })
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

    pub(crate) fn with_contract_catalog(
        mut self,
        catalog: &newengine_runtime_contract_catalog::RuntimeContractCatalog,
    ) -> Self {
        let mut resolutions = Vec::new();
        for route in &self.routes {
            let (reference, version) = match (route.contract_id.as_deref(), route.contract_version)
            {
                (Some(reference), version) => (Some(reference.to_owned()), version),
                (None, None) => route
                    .provider_abi
                    .as_deref()
                    .and_then(newengine_service_api::parse_versioned_contract_id)
                    .map(|(reference, version)| (Some(reference), Some(version)))
                    .unwrap_or((None, None)),
                _ => (None, None),
            };
            let Some(reference) = reference else {
                continue;
            };
            let Some(entry) = catalog.resolve_contract_reference(&reference) else {
                continue;
            };
            resolutions.push(runtime_contract_resolution(
                RuntimeContractResolutionInput {
                    subject: CompositionContractResolutionSubject::Candidate,
                    gateway_id: &route.gateway_id,
                    candidate_id: Some(route.selection_key.clone()),
                    capability_id: route.backend_capability_id.clone(),
                    reference: &reference,
                    min_version: version.unwrap_or(0),
                    max_version: version,
                },
                entry,
            ));
        }
        for gateway in self.plan.explanation().gateways() {
            for requirement in &gateway.requirements {
                let Some(reference) = requirement.contract_id.as_deref() else {
                    continue;
                };
                let Some(entry) = catalog.resolve_contract_reference(reference) else {
                    continue;
                };
                resolutions.push(runtime_contract_resolution(
                    RuntimeContractResolutionInput {
                        subject: CompositionContractResolutionSubject::Requirement,
                        gateway_id: &gateway.gateway_id,
                        candidate_id: None,
                        capability_id: requirement.capability_id.clone(),
                        reference,
                        min_version: requirement.min_contract_version,
                        max_version: requirement.max_contract_version,
                    },
                    entry,
                ));
            }
        }
        self.plan = std::mem::take(&mut self.plan).with_contract_resolutions(resolutions);
        self
    }

    pub(crate) fn composition_explanation(&self) -> &CompositionExplanationGraph {
        self.plan.explanation()
    }

    pub(crate) fn composition_plan(&self) -> &CompositionPlan {
        &self.plan
    }

    pub(crate) fn routes(&self) -> &[ActiveGatewayRoute] {
        &self.routes
    }

    #[cfg(test)]
    pub(crate) fn route_diagnostics(&self, gateway_id: &str) -> GatewayRouteDiagnostics {
        let explanation = self.plan.explanation().gateway(gateway_id);
        let active_key = explanation.and_then(|gateway| {
            gateway
                .candidates
                .iter()
                .find(|candidate| {
                    candidate.disposition == CompositionCandidateDisposition::Selected
                })
                .map(|candidate| candidate.candidate_id.as_str())
        });
        let shadowed_keys = explanation
            .into_iter()
            .flat_map(|gateway| gateway.candidates.iter())
            .filter(|candidate| candidate.disposition == CompositionCandidateDisposition::Shadowed)
            .map(|candidate| candidate.candidate_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let active_route = active_key.and_then(|key| {
            self.routes
                .iter()
                .find(|route| route.selection_key == key)
                .cloned()
        });
        let shadowed_routes = self
            .routes
            .iter()
            .filter(|route| shadowed_keys.contains(route.selection_key.as_str()))
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
