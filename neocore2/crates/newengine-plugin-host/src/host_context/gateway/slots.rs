use super::super::state::{
    bump_services_generation, ctx, EngineCapabilitySlotEntry, EngineCapabilitySlotSnapshot,
};
use super::registry::gateway_registry_snapshot;

/// Declares an engine-facing capability slot without constructing an implementation.
///
/// Slots are composition metadata. A plugin or runtime provider occupies the slot
/// only by registering a normal gateway route for the same `gateway_id`.
pub fn declare_engine_composition(
    composition: newengine_service_api::EngineCompositionSpec,
) -> Result<(), String> {
    if composition.id.trim().is_empty() {
        return Err("engine composition id must not be empty".to_owned());
    }
    for slot in composition.slots {
        declare_engine_capability_slot(
            slot.gateway_id,
            slot.service_kind,
            slot.required,
            composition.id,
        )?;
    }
    Ok(())
}

pub fn declare_engine_capability_slot<S>(
    gateway_id: &str,
    service_kind: S,
    required: bool,
    declared_by: &str,
) -> Result<(), String>
where
    S: AsRef<str>,
{
    let gateway_id = newengine_service_api::normalize_engine_gateway_id(gateway_id)
        .ok_or_else(|| format!("capability slot gateway id is invalid: '{gateway_id}'"))?;
    let raw_service_kind = service_kind.as_ref();
    let service_kind = newengine_service_api::normalize_service_kind(raw_service_kind)
        .ok_or_else(|| format!("capability slot service kind is invalid: '{raw_service_kind}'"))?;
    if !newengine_service_api::engine_gateway_matches_service_kind(&gateway_id, &service_kind) {
        return Err(format!(
            "capability slot service_kind/domain mismatch: gateway='{gateway_id}' service_kind='{service_kind}' expected='{}'",
            newengine_service_api::service_kind_from_engine_gateway_id(&gateway_id)
                .unwrap_or_else(|| "<invalid>".to_owned())
        ));
    }
    let declared_by = declared_by.trim();
    if declared_by.is_empty() {
        return Err("capability slot declared_by must not be empty".to_owned());
    }

    let c = ctx();
    let mut slots = match c.capability_slots.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    match slots.get_mut(&gateway_id) {
        Some(existing) => {
            if existing.service_kind != service_kind {
                return Err(format!(
                    "capability slot '{}' already declared with service_kind='{}', cannot redeclare as '{}'",
                    gateway_id, existing.service_kind, service_kind
                ));
            }
            existing.required |= required;
        }
        None => {
            slots.insert(
                gateway_id.clone(),
                EngineCapabilitySlotEntry {
                    gateway_id: gateway_id.clone(),
                    service_kind,
                    required,
                    declared_by: declared_by.to_owned(),
                },
            );
        }
    }
    drop(slots);
    bump_services_generation();
    Ok(())
}

/// Returns both empty and occupied capability slots.
///
/// Provider routes that were not explicitly declared by a composition are still
/// surfaced as implicit occupied slots so diagnostics never hide live providers.
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
        .map(|slot| slot.gateway_id.clone())
        .chain(registry.gateway_ids())
        .collect::<Vec<_>>();
    slot_ids.sort();
    slot_ids.dedup();

    slot_ids
        .into_iter()
        .map(|gateway_id| {
            let declaration = declared.iter().find(|slot| slot.gateway_id == gateway_id);
            let active = registry.resolve_route(&gateway_id);
            let service_kind = declaration
                .map(|slot| slot.service_kind.clone())
                .or_else(|| active.map(|route| route.service_kind.clone()))
                .unwrap_or_else(|| {
                    newengine_service_api::service_kind_from_engine_gateway_id(&gateway_id)
                        .unwrap_or_else(|| "unknown".to_owned())
                });
            EngineCapabilitySlotSnapshot {
                gateway_id,
                service_kind,
                required: declaration.is_some_and(|slot| slot.required),
                declared_by: declaration
                    .map(|slot| slot.declared_by.clone())
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

/// Enforces only slots explicitly marked `required` by the selected composition.
/// Empty optional slots are a valid host state.
pub fn validate_required_engine_capability_slots() -> Result<(), String> {
    let missing = list_engine_capability_slots()
        .into_iter()
        .filter(|slot| slot.required && slot.state == "empty")
        .map(|slot| {
            format!(
                "{}(kind={} declared_by={})",
                slot.gateway_id, slot.service_kind, slot.declared_by
            )
        })
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "required engine capability slot(s) are empty after plugin composition: {}",
            missing.join(", ")
        ))
    }
}
