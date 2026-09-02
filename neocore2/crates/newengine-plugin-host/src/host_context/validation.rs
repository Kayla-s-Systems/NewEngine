use newengine_plugin_api::{CapabilityKind, CapabilityRole, PluginDescriptor, PluginDescriptorV2};

use super::state::{bump_services_generation, ctx};

#[inline]
pub(crate) fn effective_provider_origin(
    descriptor: &PluginDescriptor,
    descriptor_v2: Option<&PluginDescriptorV2>,
    default_origin: crate::service_gateway::GatewayProviderOrigin,
) -> crate::service_gateway::GatewayProviderOrigin {
    let typed_owned;
    let typed = match descriptor_v2 {
        Some(descriptor) => descriptor,
        None => {
            typed_owned = PluginDescriptorV2::from_legacy(descriptor);
            &typed_owned
        }
    };

    let id = typed.id.as_str().to_ascii_lowercase();
    if id.contains("null") {
        return crate::service_gateway::GatewayProviderOrigin::NullProvider;
    }

    for cap in typed.capabilities.iter() {
        if cap.role != CapabilityRole::Provides {
            continue;
        }
        let abi_stable::std_types::ROption::RSome(route) = cap.route.clone() else {
            continue;
        };
        let backend_is_null = match route.backend {
            abi_stable::std_types::ROption::RSome(value) => value.eq_ignore_ascii_case("null"),
            abi_stable::std_types::ROption::RNone => false,
        };
        let mode_is_headless = match route.mode {
            abi_stable::std_types::ROption::RSome(value) => value.eq_ignore_ascii_case("headless"),
            abi_stable::std_types::ROption::RNone => false,
        };
        if backend_is_null || mode_is_headless {
            return crate::service_gateway::GatewayProviderOrigin::NullProvider;
        }
    }

    default_origin
}

/// Registers a plugin descriptor (host-owned metadata) for runtime validation.
///
/// Called by the plugin loader *before* `init()` so that service registrations during
/// init can be validated against declared capabilities.
pub(crate) fn register_plugin_descriptor(
    plugin_id: &str,
    d: PluginDescriptor,
    d_v2: Option<PluginDescriptorV2>,
    origin: crate::service_gateway::GatewayProviderOrigin,
) -> crate::service_gateway::GatewayProviderOrigin {
    let typed_for_origin = d_v2.as_ref();
    let origin = effective_provider_origin(&d, typed_for_origin, origin);
    match super::lifecycle::stage_plugin_descriptor_registration(
        plugin_id,
        d.clone(),
        d_v2.clone(),
        origin,
    ) {
        Ok(true) => return origin,
        Ok(false) => {}
        Err(error) => {
            newengine_ulog_api::ulog::error!(
                "provider transaction: descriptor staging failed owner='{}' err='{}'",
                plugin_id,
                error
            );
            return origin;
        }
    }
    let c = ctx();
    {
        let mut g = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.insert(plugin_id.to_owned(), d.clone());
    }
    {
        let mut g = match c.plugin_descriptors_v2.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.insert(
            plugin_id.to_owned(),
            d_v2.unwrap_or_else(|| PluginDescriptorV2::from_legacy(&d)),
        );
    }

    {
        let mut g = match c.plugin_origins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.insert(plugin_id.to_owned(), origin);
    }

    bump_services_generation();
    origin
}

#[inline]
fn capability_composition_key(id: &str, kind: CapabilityKind) -> String {
    format!("__capability__/{}/{}", kind as u8, id.trim())
}

fn candidate_from_typed_capability(
    owner_id: &str,
    capability: &newengine_plugin_api::CapabilityDescV2,
) -> Option<newengine_service_api::CompositionCandidate> {
    if capability.role != CapabilityRole::Provides {
        return None;
    }
    let mut candidate = newengine_service_api::CompositionCandidate::new(
        capability_composition_key(capability.id.as_str(), capability.kind),
        format!(
            "{}::{}::{}::v{}",
            owner_id, capability.kind as u8, capability.id, capability.version
        ),
        owner_id,
        0,
        0,
        0,
    )
    .with_capability(capability.id.to_string())
    .with_capability_version(capability.version)
    .with_tags(capability.tags.iter().map(|tag| tag.as_str().to_owned()));

    if let abi_stable::std_types::ROption::RSome(contract) = capability.contract.clone() {
        candidate = match contract.version {
            abi_stable::std_types::ROption::RSome(version) => {
                candidate.with_contract(contract.id.to_string(), version)
            }
            abi_stable::std_types::ROption::RNone => {
                candidate.with_contract_id(contract.id.to_string())
            }
        };
    }
    Some(candidate)
}

fn route_alias_candidate_from_typed_capability(
    owner_id: &str,
    capability: &newengine_plugin_api::CapabilityDescV2,
) -> Option<newengine_service_api::CompositionCandidate> {
    if capability.role != CapabilityRole::Provides {
        return None;
    }
    let abi_stable::std_types::ROption::RSome(route) = capability.route.clone() else {
        return None;
    };
    let gateway_id = route.engine_gateway.as_str().trim();
    if gateway_id.is_empty() {
        return None;
    }
    Some(
        newengine_service_api::CompositionCandidate::new(
            capability_composition_key(gateway_id, CapabilityKind::ServiceV1),
            format!("{}::gateway::{}", owner_id, gateway_id),
            owner_id,
            route.backend_priority,
            0,
            0,
        )
        .with_capability(gateway_id.to_owned())
        .with_capability_version(1)
        .with_tags(capability.tags.iter().map(|tag| tag.as_str().to_owned())),
    )
}

fn candidate_from_active_gateway_route(
    route: &super::state::EngineGatewayRouteSnapshot,
) -> newengine_service_api::CompositionCandidate {
    newengine_service_api::CompositionCandidate::new(
        capability_composition_key(&route.gateway_id, CapabilityKind::ServiceV1),
        format!(
            "active-gateway::{}::{}::{}",
            route.gateway_id, route.provider_owner_id, route.provider_service_id
        ),
        route.provider_owner_id.clone(),
        route.backend_priority,
        0,
        0,
    )
    .with_capability(route.gateway_id.clone())
    .with_capability_version(1)
}

fn requirement_from_typed_capability(
    owner_id: &str,
    capability: &newengine_plugin_api::CapabilityDescV2,
) -> Option<newengine_service_api::CompositionRequirement> {
    if capability.role != CapabilityRole::Requires {
        return None;
    }
    let requirement = match capability.requirement.clone() {
        abi_stable::std_types::ROption::RSome(requirement) => requirement,
        abi_stable::std_types::ROption::RNone => {
            newengine_plugin_api::CapabilityRequirementDescV2::at_least(capability.version)
        }
    };
    let (contract_id, min_contract_version, max_contract_version) =
        match capability.contract.clone() {
            abi_stable::std_types::ROption::RSome(contract) => match contract.version {
                abi_stable::std_types::ROption::RSome(version) => {
                    (Some(contract.id.to_string()), version, Some(version))
                }
                abi_stable::std_types::ROption::RNone => (Some(contract.id.to_string()), 0, None),
            },
            abi_stable::std_types::ROption::RNone => (None, 0, None),
        };

    Some(newengine_service_api::CompositionRequirement {
        capability_id: capability.id.to_string(),
        gateway_id: capability_composition_key(capability.id.as_str(), capability.kind),
        service_kind: "capability".to_owned(),
        level: newengine_service_api::RequirementStrength::Required,
        min_capability_version: requirement.min_version,
        max_capability_version: requirement.max_version.into_option(),
        contract_id,
        min_contract_version,
        max_contract_version,
        required_tags: requirement
            .required_tags
            .iter()
            .map(|tag| tag.as_str().to_owned())
            .collect(),
        preferred_tags: requirement
            .preferred_tags
            .iter()
            .map(|tag| tag.as_str().to_owned())
            .collect(),
        conflict_tags: requirement
            .forbidden_tags
            .iter()
            .map(|tag| tag.as_str().to_owned())
            .collect(),
        fallback_provider_ids: Vec::new(),
        min_cardinality: 1,
        max_cardinality: 1,
        declared_by: owner_id.to_owned(),
    })
}

pub(crate) fn capability_provider_candidates() -> Vec<newengine_service_api::CompositionCandidate> {
    let c = ctx();
    let descriptors = match c.plugin_descriptors_v2.lock() {
        Ok(value) => value.values().cloned().collect::<Vec<_>>(),
        Err(poisoned) => poisoned.into_inner().values().cloned().collect::<Vec<_>>(),
    };
    let services = match c.services.lock() {
        Ok(value) => value.keys().cloned().collect::<Vec<String>>(),
        Err(poisoned) => poisoned.into_inner().keys().cloned().collect::<Vec<_>>(),
    };

    let mut out = Vec::new();
    for descriptor in descriptors {
        let owner_id = descriptor.id.as_str();
        for capability in descriptor.capabilities.iter() {
            if let Some(candidate) = candidate_from_typed_capability(owner_id, capability) {
                out.push(candidate);
            }
            if let Some(candidate) =
                route_alias_candidate_from_typed_capability(owner_id, capability)
            {
                out.push(candidate);
            }
        }
    }

    for service_id in services
        .into_iter()
        .chain(std::iter::once("host.services.v1".to_owned()))
    {
        out.push(
            newengine_service_api::CompositionCandidate::new(
                capability_composition_key(&service_id, CapabilityKind::ServiceV1),
                format!("host-service::{service_id}"),
                "host",
                0,
                0,
                0,
            )
            .with_capability(service_id)
            .with_capability_version(1),
        );
    }

    // Service requirements authored against stable `engine.*` gateways must be
    // satisfiable by host-owned gateway routes as well as plugin-declared routes.
    // Previously only concrete service ids (for example `threading.api`) were added
    // here, so a valid host route `engine.threading -> threading.api` was invisible
    // to plugin requirement validation and StarProfiler was incorrectly disabled.
    for gateway_id in super::gateway::active_engine_gateways() {
        if let Some(route) = super::gateway::active_engine_gateway_route(&gateway_id) {
            out.push(candidate_from_active_gateway_route(&route));
        }
    }

    out.push(
        newengine_service_api::CompositionCandidate::new(
            capability_composition_key("host.events.v1", CapabilityKind::EventsV1),
            "host-events::host.events.v1",
            "host",
            0,
            0,
            0,
        )
        .with_capability("host.events.v1")
        .with_capability_version(1),
    );
    out
}

pub(crate) fn capability_provider_candidates_with_descriptor(
    descriptor: &PluginDescriptorV2,
) -> Vec<newengine_service_api::CompositionCandidate> {
    let mut out = capability_provider_candidates();
    let owner_id = descriptor.id.as_str();
    for capability in descriptor.capabilities.iter() {
        if let Some(candidate) = candidate_from_typed_capability(owner_id, capability) {
            out.push(candidate);
        }
        if let Some(candidate) = route_alias_candidate_from_typed_capability(owner_id, capability) {
            out.push(candidate);
        }
    }
    out
}

pub(crate) fn missing_typed_descriptor_requirements(
    descriptor: &PluginDescriptorV2,
    candidates: &[newengine_service_api::CompositionCandidate],
) -> Vec<String> {
    let requirements = descriptor
        .capabilities
        .iter()
        .filter_map(|capability| {
            requirement_from_typed_capability(descriptor.id.as_str(), capability)
        })
        .collect::<Vec<_>>();
    if requirements.is_empty() {
        return Vec::new();
    }

    let matrix = newengine_service_api::CapabilityMatrix::new(requirements);
    let plan = newengine_service_api::CompositionSolver::resolve_input(
        newengine_service_api::CompositionSolverInput {
            candidates: candidates.to_vec(),
            capability_matrix: matrix.clone(),
        },
    );

    let mut out = plan
        .unsatisfied()
        .iter()
        .filter_map(|missing| matrix.requirement(&missing.gateway_id))
        .map(|requirement| {
            let max = requirement
                .max_capability_version
                .map(|value| value.to_string())
                .unwrap_or_else(|| "*".to_owned());
            let mut constraints = vec![format!(
                "version={}..{}",
                requirement.min_capability_version, max
            )];
            if let Some(contract_id) = requirement.contract_id.as_deref() {
                let contract_max = requirement
                    .max_contract_version
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "*".to_owned());
                constraints.push(format!(
                    "contract={}@{}..{}",
                    contract_id, requirement.min_contract_version, contract_max
                ));
            }
            if !requirement.required_tags.is_empty() {
                constraints.push(format!(
                    "required_tags={}",
                    requirement.required_tags.join("|")
                ));
            }
            if !requirement.preferred_tags.is_empty() {
                constraints.push(format!(
                    "preferred_tags={}",
                    requirement.preferred_tags.join("|")
                ));
            }
            if !requirement.conflict_tags.is_empty() {
                constraints.push(format!(
                    "forbidden_tags={}",
                    requirement.conflict_tags.join("|")
                ));
            }
            format!("{}({})", requirement.capability_id, constraints.join(" "))
        })
        .collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}

/// Returns:
/// - `Some(true)` if the plugin has a descriptor and declares `Provides(ServiceV1, service_id)`.
/// - `Some(false)` if the plugin has a descriptor but does not declare that capability.
/// - `None` if the plugin has no known descriptor (ABI v1 or loader did not register it).
pub(crate) fn plugin_declares_provided_service(plugin_id: &str, service_id: &str) -> Option<bool> {
    if let Some(declared) = super::lifecycle::staged_plugin_declares_service(plugin_id, service_id)
    {
        return Some(declared);
    }
    let c = ctx();
    let g = c.plugin_descriptors.lock().ok()?;
    let d = g.get(plugin_id)?;

    for cap in d.capabilities.iter() {
        if cap.role != CapabilityRole::Provides {
            continue;
        }
        if cap.kind != CapabilityKind::ServiceV1 {
            continue;
        }
        if cap.id.as_str() == service_id {
            return Some(true);
        }
    }

    Some(false)
}

#[cfg(test)]
mod typed_requirement_tests {
    include!("validation/tests.rs");
}
