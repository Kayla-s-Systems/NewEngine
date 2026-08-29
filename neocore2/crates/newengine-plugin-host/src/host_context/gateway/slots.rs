use super::super::state::{
    bump_services_generation, ctx, DeclaredCompositionPolicy, EngineCapabilitySlotEntry,
    EngineCapabilitySlotSnapshot,
};
use super::registry::gateway_registry_snapshot;
use newengine_service_api::{
    CapabilityId, CapabilityMatrix, CapabilityRequirement, CapabilityRequirementLevel,
    CompositionRequirement, RequirementStrength,
};

/// Declares a complete composition requirement matrix without constructing providers.
pub fn declare_engine_composition(
    composition: newengine_service_api::EngineCompositionSpec,
) -> Result<(), String> {
    if composition.id.trim().is_empty() {
        return Err("engine composition id must not be empty".to_owned());
    }

    let mut preferred_tags = composition
        .preferred_tags
        .iter()
        .map(|tag| tag.as_str().to_owned())
        .collect::<Vec<_>>();
    preferred_tags.sort();
    preferred_tags.dedup();
    let mut forbidden_tags = composition
        .forbidden_tags
        .iter()
        .map(|tag| tag.as_str().to_owned())
        .collect::<Vec<_>>();
    forbidden_tags.sort();
    forbidden_tags.dedup();
    {
        let c = ctx();
        let mut policy = match c.composition_policy.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        *policy = DeclaredCompositionPolicy {
            preferred_tags,
            forbidden_tags,
        };
    }
    // A policy-only composition must invalidate the immutable gateway plan too.
    bump_services_generation();

    for requirement in composition.requirements {
        declare_engine_capability_requirement(*requirement, composition.id)?;
    }
    Ok(())
}

pub(crate) fn declared_engine_composition_matrix() -> CapabilityMatrix {
    let c = ctx();
    let requirements = {
        let slots = match c.capability_slots.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        slots
            .values()
            .map(|entry| entry.requirement.clone())
            .collect::<Vec<_>>()
    };
    let policy = {
        let policy = match c.composition_policy.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        policy.clone()
    };
    CapabilityMatrix::new(requirements)
        .with_preferred_tags(policy.preferred_tags)
        .with_forbidden_tags(policy.forbidden_tags)
}

pub fn engine_composition_allows_system_tags(tags: &[String]) -> bool {
    declared_engine_composition_matrix().allows_system_tags(tags)
}

#[inline]
pub fn engine_composition_has_forbidden_system_tags() -> bool {
    !declared_engine_composition_matrix()
        .conflict_tags()
        .is_empty()
}

pub fn declare_engine_capability_requirement(
    spec: CapabilityRequirement,
    declared_by: &str,
) -> Result<(), String> {
    crate::host_context::reject_topology_mutation_from_host_callback(
        "declare_engine_capability_requirement",
    )?;
    let gateway_id =
        newengine_service_api::normalize_engine_gateway_id(spec.capability.gateway_id())
            .ok_or_else(|| {
                format!(
                    "capability '{}' is bound to invalid engine gateway '{}'",
                    spec.capability.as_str(),
                    spec.capability.gateway_id()
                )
            })?;
    let service_kind = newengine_service_api::normalize_service_kind(
        spec.capability.service_kind(),
    )
    .ok_or_else(|| {
        format!(
            "capability '{}' is bound to invalid service kind '{}'",
            spec.capability.as_str(),
            spec.capability.service_kind()
        )
    })?;
    if !newengine_service_api::engine_gateway_matches_service_kind(&gateway_id, &service_kind) {
        return Err(format!(
            "capability '{}' route binding mismatch: gateway='{gateway_id}' service_kind='{service_kind}' expected='{}'",
            spec.capability.as_str(),
            newengine_service_api::service_kind_from_engine_gateway_id(&gateway_id)
                .unwrap_or_else(|| "<invalid>".to_owned())
        ));
    }

    let declared_by = declared_by.trim();
    if declared_by.is_empty() {
        return Err("capability requirement declared_by must not be empty".to_owned());
    }

    let mut incoming = CompositionRequirement::from_spec(&spec, declared_by);
    incoming.gateway_id = gateway_id.clone();
    incoming.service_kind = service_kind;

    let c = ctx();
    let mut slots = match c.capability_slots.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    match slots.get(&gateway_id) {
        Some(existing) if existing.requirement.service_kind != incoming.service_kind => {
            return Err(format!(
                "capability requirement '{}' already declared with service_kind='{}', cannot redeclare as '{}'",
                gateway_id, existing.requirement.service_kind, incoming.service_kind
            ));
        }
        Some(existing) => {
            let merged = CapabilityMatrix::new(vec![existing.requirement.clone(), incoming]);
            let requirement = merged
                .requirement(&gateway_id)
                .cloned()
                .ok_or_else(|| format!("failed to merge capability requirement '{gateway_id}'"))?;
            slots.insert(
                gateway_id.clone(),
                EngineCapabilitySlotEntry { requirement },
            );
        }
        None => {
            slots.insert(
                gateway_id,
                EngineCapabilitySlotEntry {
                    requirement: incoming,
                },
            );
        }
    }
    drop(slots);
    bump_services_generation();
    Ok(())
}

/// Transitional V1 declaration API. New code should construct a logical
/// `CapabilityRequirement` from a domain-owned `BackendServiceSpec::capability()`.
pub fn declare_engine_capability_slot<S>(
    gateway_id: &str,
    service_kind: S,
    required: bool,
    declared_by: &str,
) -> Result<(), String>
where
    S: AsRef<str>,
{
    let gateway_id = Box::leak(gateway_id.to_owned().into_boxed_str());
    let service_kind = Box::leak(service_kind.as_ref().to_owned().into_boxed_str());
    let capability = CapabilityId::new(gateway_id, gateway_id, service_kind);
    let spec = CapabilityRequirement::new(
        capability,
        if required {
            RequirementStrength::Required
        } else {
            RequirementStrength::Optional
        },
    );
    declare_engine_capability_requirement(spec, declared_by)
}

pub fn list_engine_capability_slots() -> Vec<EngineCapabilitySlotSnapshot> {
    let registry = gateway_registry_snapshot();
    let c = ctx();
    let declared = {
        let slots = match c.capability_slots.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        slots.values().cloned().collect::<Vec<_>>()
    };

    let mut slot_ids = declared
        .iter()
        .map(|slot| slot.requirement.gateway_id.clone())
        .chain(registry.gateway_ids())
        .collect::<Vec<_>>();
    slot_ids.sort();
    slot_ids.dedup();

    slot_ids
        .into_iter()
        .map(|gateway_id| {
            let declaration = declared
                .iter()
                .find(|slot| slot.requirement.gateway_id == gateway_id)
                .map(|slot| &slot.requirement);
            let active = registry.resolve_route(&gateway_id);
            let service_kind = declaration
                .map(|requirement| requirement.service_kind.clone())
                .or_else(|| active.map(|route| route.service_kind.clone()))
                .unwrap_or_else(|| {
                    newengine_service_api::service_kind_from_engine_gateway_id(&gateway_id)
                        .unwrap_or_else(|| "unknown".to_owned())
                });
            EngineCapabilitySlotSnapshot {
                gateway_id,
                service_kind,
                required: declaration.is_some_and(|requirement| requirement.level.is_required()),
                requirement_level: declaration
                    .map(|requirement| requirement.level)
                    .unwrap_or(CapabilityRequirementLevel::Optional),
                contract_id: declaration.and_then(|requirement| requirement.contract_id.clone()),
                min_contract_version: declaration
                    .map(|requirement| requirement.min_contract_version)
                    .unwrap_or(0),
                max_contract_version: declaration
                    .and_then(|requirement| requirement.max_contract_version),
                required_tags: declaration
                    .map(|requirement| requirement.required_tags.clone())
                    .unwrap_or_default(),
                preferred_tags: declaration
                    .map(|requirement| requirement.preferred_tags.clone())
                    .unwrap_or_default(),
                conflict_tags: declaration
                    .map(|requirement| requirement.conflict_tags.clone())
                    .unwrap_or_default(),
                fallback_provider_ids: declaration
                    .map(|requirement| requirement.fallback_provider_ids.clone())
                    .unwrap_or_default(),
                min_cardinality: declaration
                    .map(|requirement| requirement.min_cardinality)
                    .unwrap_or(0),
                max_cardinality: declaration
                    .map(|requirement| requirement.max_cardinality)
                    .unwrap_or(1),
                declared_by: declaration
                    .map(|requirement| requirement.declared_by.clone())
                    .unwrap_or_else(|| "implicit-provider-route".to_owned()),
                state: if active.is_some() {
                    "occupied"
                } else {
                    "empty"
                }
                .to_owned(),
                provider_service_id: active.map(|route| route.provider_service_id.clone()),
                provider_owner_id: active.map(|route| route.provider_owner_id.clone()),
                provider_origin: active.map(|route| route.origin.as_str().to_owned()),
                backend_capability_id: active.map(|route| route.backend_capability_id.clone()),
            }
        })
        .collect()
}

pub fn validate_required_engine_capability_slots() -> Result<(), String> {
    gateway_registry_snapshot().validate_required_requirements()
}
