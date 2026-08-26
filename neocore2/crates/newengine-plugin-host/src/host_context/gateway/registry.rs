use std::cell::Cell;
use std::sync::{atomic::Ordering, Arc};

use super::super::state::{
    ctx, current_host_context, services_generation, GatewayRegistryCache, HostContext,
};

thread_local! {
    // Pure recursion guard. Unlike semantic diagnostics state, this is execution-local.
    static IN_GATEWAY_DIAGNOSTIC: Cell<bool> = const { Cell::new(false) };
}

#[inline]
pub(super) fn emit_gateway_diagnostic(f: impl FnOnce()) {
    IN_GATEWAY_DIAGNOSTIC.with(|c| {
        if c.get() {
            return;
        }
        c.set(true);
        struct Restore<'a>(&'a Cell<bool>);
        impl<'a> Drop for Restore<'a> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _restore = Restore(c);
        f();
    });
}

pub(super) fn should_emit_gateway_resolution(gateway_id: &str, resolution: &str) -> bool {
    let context = ctx();
    let mut state = match context.gateway_resolution_diagnostics.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    };
    match state.get(gateway_id) {
        Some(previous) if previous == resolution => false,
        _ => {
            state.insert(gateway_id.to_owned(), resolution.to_owned());
            true
        }
    }
}

fn registered_services_snapshot(
    c: &HostContext,
) -> Vec<crate::service_gateway::RegisteredServiceFact> {
    let services = match c.services.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    services
        .iter()
        .map(|(service_id, entry)| {
            crate::service_gateway::RegisteredServiceFact::new(
                service_id.clone(),
                entry.owner_plugin_id.clone(),
            )
        })
        .collect::<Vec<_>>()
}

fn gateway_provider_routes_snapshot(
    c: &HostContext,
) -> Vec<crate::service_gateway::GatewayProviderRouteFact> {
    let gateways = match c.gateway_provider_routes.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    gateways
        .values()
        .map(|entry| {
            crate::service_gateway::GatewayProviderRouteFact::new_dynamic_with_origin(
                entry.gateway_id.clone(),
                entry.service_kind.clone(),
                entry.provider_service_id.clone(),
                entry.provider_route_id.clone(),
                entry.provider_abi.clone(),
                entry.provider_owner_id.clone(),
                entry.backend_capability_id.clone(),
                entry.backend_priority,
                entry.origin,
                entry.system_tags.clone(),
            )
        })
        .collect::<Vec<_>>()
}

fn selection_policies_snapshot(c: &HostContext) -> Vec<crate::service_gateway::GatewayPolicyFact> {
    let policies = match c.gateway_selection_policies.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    policies
        .values()
        .map(|policy| crate::service_gateway::GatewayPolicyFact {
            gateway_id: policy.gateway_id.clone(),
            override_mode: None,
            system_tags: Vec::new(),
            preferred_system_tags: policy.preferred_system_tags.clone(),
            forbidden_system_tags: policy.forbidden_system_tags.clone(),
            preference_bonus: policy.preference_bonus,
            owner_id: policy.owner_id.clone(),
        })
        .collect::<Vec<_>>()
}

#[derive(Clone)]
pub(crate) struct CompositionPlanningSnapshot {
    pub(crate) services: Vec<crate::service_gateway::RegisteredServiceFact>,
    pub(crate) gateway_provider_routes: Vec<crate::service_gateway::GatewayProviderRouteFact>,
    pub(crate) selection_policies: Vec<crate::service_gateway::GatewayPolicyFact>,
    pub(crate) capability_matrix: newengine_service_api::CapabilityMatrix,
}

pub(crate) fn composition_planning_snapshot() -> CompositionPlanningSnapshot {
    let c = ctx();
    CompositionPlanningSnapshot {
        services: registered_services_snapshot(&c),
        gateway_provider_routes: gateway_provider_routes_snapshot(&c),
        selection_policies: selection_policies_snapshot(&c),
        capability_matrix: super::slots::declared_engine_composition_matrix(),
    }
}

fn build_gateway_registry_snapshot() -> crate::service_gateway::ActiveGatewayRegistry {
    let c = ctx();
    let planning = composition_planning_snapshot();

    let plugin_origins = {
        let origins = match c.plugin_origins.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        origins.clone()
    };
    let descriptors = {
        let descriptors = match c.plugin_descriptors.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        let descriptors_v2 = match c.plugin_descriptors_v2.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        descriptors
            .iter()
            .map(|(plugin_id, descriptor)| {
                crate::service_gateway::PluginDescriptorFact::new_with_v2(
                    plugin_id.clone(),
                    descriptor.clone(),
                    descriptors_v2.get(plugin_id).cloned(),
                    plugin_origins
                        .get(plugin_id)
                        .copied()
                        .unwrap_or(crate::service_gateway::GatewayProviderOrigin::GamePlugin),
                )
            })
            .collect::<Vec<_>>()
    };
    let frozen_plan = {
        let slot = match c.frozen_composition_plan.read() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        slot.clone()
    };

    let contract_catalog = {
        let catalog = match c.runtime_contract_catalog.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        catalog.clone()
    };

    crate::service_gateway::ActiveGatewayRegistry::from_facts_with_policy_matrix_and_plan(
        &descriptors,
        &planning.services,
        &planning.gateway_provider_routes,
        &planning.selection_policies,
        planning.capability_matrix,
        frozen_plan.as_deref(),
    )
    .with_contract_catalog(&contract_catalog)
}

pub(super) fn gateway_registry_snapshot() -> Arc<crate::service_gateway::ActiveGatewayRegistry> {
    let c = ctx();
    loop {
        let generation_before = c.services_generation.load(Ordering::Acquire);
        if generation_before & 1 != 0 {
            std::thread::yield_now();
            continue;
        }
        {
            let cache = match c.gateway_registry_cache.lock() {
                Ok(v) => v,
                Err(e) => e.into_inner(),
            };
            if let Some(cache) = cache.as_ref() {
                if cache.generation == generation_before {
                    return Arc::clone(&cache.registry);
                }
            }
        }

        let registry = Arc::new(build_gateway_registry_snapshot());
        let generation_after = c.services_generation.load(Ordering::Acquire);
        if generation_before != generation_after || generation_after & 1 != 0 {
            continue;
        }
        {
            let mut cache = match c.gateway_registry_cache.lock() {
                Ok(v) => v,
                Err(e) => e.into_inner(),
            };
            *cache = Some(GatewayRegistryCache {
                generation: generation_after,
                registry: Arc::clone(&registry),
            });
        }
        return registry;
    }
}

pub(super) fn emit_gateway_route_selected(
    gateway_id: &str,
    route: &crate::service_gateway::ActiveGatewayRoute,
) {
    emit_gateway_diagnostic(|| {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.gateway.route.selected",
            "INFO",
            "Gateway route selected",
            serde_json::json!({
                "gateway_id": gateway_id,
                "provider_service_id": route.provider_service_id,
                "provider_route_id": route.provider_route_id,
                "provider_abi": route.provider_abi,
                "provider_owner_id": route.provider_owner_id,
                "backend_capability_id": route.backend_capability_id,
                "backend_priority": route.backend_priority,
                "origin": route.origin.as_str(),
                "active_score": route.active_score
            }),
        );
    });
}

pub(super) fn emit_gateway_route_missing(gateway_id: &str) {
    emit_gateway_diagnostic(|| {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.gateway.route.missing",
            "WARN",
            "Gateway route missing",
            serde_json::json!({ "gateway_id": gateway_id }),
        );
    });
}

pub(super) fn emit_gateway_route_shadowed(
    route: &crate::service_gateway::ActiveGatewayRoute,
    active: &crate::service_gateway::ActiveGatewayRoute,
) {
    emit_gateway_diagnostic(|| {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.gateway.route.shadowed",
            "INFO",
            "Gateway route shadowed",
            serde_json::json!({
                "gateway_id": route.gateway_id,
                "provider_service_id": route.provider_service_id,
                "provider_route_id": route.provider_route_id,
                "provider_abi": route.provider_abi,
                "provider_owner_id": route.provider_owner_id,
                "backend_capability_id": route.backend_capability_id,
                "active_provider_service_id": active.provider_service_id,
                "active_provider_route_id": active.provider_route_id,
                "active_provider_owner_id": active.provider_owner_id,
                "active_score": active.active_score,
                "shadowed_score": route.active_score
            }),
        );
    });
}

pub fn engine_composition_explanation() -> newengine_service_api::CompositionExplanationGraph {
    gateway_registry_snapshot()
        .composition_explanation()
        .clone()
}

pub fn explain_engine_gateway_composition(
    gateway_id: &str,
) -> Option<newengine_service_api::GatewayCompositionExplanation> {
    gateway_registry_snapshot()
        .composition_explanation()
        .gateway(gateway_id)
        .cloned()
}

pub fn engine_composition_snapshot_v1() -> newengine_service_api::CompositionSnapshotV1 {
    loop {
        let context = current_host_context();
        let generation_before = services_generation();
        if generation_before & 1 != 0 {
            std::thread::yield_now();
            continue;
        }
        let registry = gateway_registry_snapshot();
        let frozen = {
            let context_state = ctx();
            let slot = match context_state.frozen_composition_plan.read() {
                Ok(value) => value,
                Err(poisoned) => poisoned.into_inner(),
            };
            slot.is_some()
        };
        let generation_after = services_generation();
        if generation_before != generation_after || generation_after & 1 != 0 {
            continue;
        }
        let provenance = if frozen {
            newengine_service_api::CompositionSnapshotProvenanceV1::frozen(
                "host.frozen_composition_plan",
            )
        } else {
            newengine_service_api::CompositionSnapshotProvenanceV1::live("host.gateway_registry")
        };
        return newengine_service_api::CompositionSnapshotV1::from_plan(
            context.instance_id(),
            generation_after / 2,
            generation_after,
            provenance,
            registry.composition_plan(),
        );
    }
}

pub fn engine_composition_snapshot_v1_json() -> Result<String, String> {
    engine_composition_snapshot_v1()
        .to_json()
        .map_err(|error| format!("composition.snapshot_v1 serialization failed: {error}"))
}

pub(crate) fn active_engine_gateways() -> Vec<String> {
    gateway_registry_snapshot().gateway_ids()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_snapshot_v1_tracks_instance_epoch_generation_and_provenance() {
        let handle = crate::host_context::create_host_context_with_environment_snapshot(Vec::<(
            std::ffi::OsString,
            std::ffi::OsString,
        )>::new(
        ));
        crate::host_context::with_host_context(&handle, || {
            let live = engine_composition_snapshot_v1();
            assert_eq!(
                live.schema,
                newengine_service_api::COMPOSITION_SNAPSHOT_SCHEMA_V1
            );
            assert_eq!(live.instance_id, handle.instance_id());
            assert_eq!(live.composition_epoch, live.topology_generation / 2);
            assert_eq!(live.topology_generation & 1, 0);
            assert_eq!(
                live.provenance.mode,
                newengine_service_api::CompositionPlanModeV1::Live
            );

            let frozen_plan = newengine_service_api::CompositionSolver::resolve(Vec::<
                newengine_service_api::CompositionCandidate,
            >::new(
            ));
            handle
                .freeze_composition_plan(frozen_plan)
                .expect("freeze composition plan");
            let frozen = engine_composition_snapshot_v1();
            assert_eq!(frozen.instance_id, live.instance_id);
            assert!(frozen.composition_epoch > live.composition_epoch);
            assert!(frozen.topology_generation > live.topology_generation);
            assert_eq!(
                frozen.provenance.mode,
                newengine_service_api::CompositionPlanModeV1::Frozen
            );
            let json = engine_composition_snapshot_v1_json().expect("snapshot json");
            let decoded = newengine_service_api::CompositionSnapshotV1::from_json(&json)
                .expect("decode runtime snapshot");
            assert_eq!(decoded, frozen);
        });
    }

    #[test]
    fn gateway_resolution_deduplication_is_instance_scoped() {
        let a = crate::host_context::create_host_context();
        let b = crate::host_context::create_host_context();

        crate::host_context::with_host_context(&a, || {
            assert!(should_emit_gateway_resolution(
                "engine.render",
                "provider-a"
            ));
            assert!(!should_emit_gateway_resolution(
                "engine.render",
                "provider-a"
            ));
        });

        crate::host_context::with_host_context(&b, || {
            assert!(should_emit_gateway_resolution(
                "engine.render",
                "provider-a"
            ));
            assert!(!should_emit_gateway_resolution(
                "engine.render",
                "provider-a"
            ));
        });

        crate::host_context::with_host_context(&a, || {
            assert!(should_emit_gateway_resolution(
                "engine.render",
                "provider-b"
            ));
        });
    }
}
