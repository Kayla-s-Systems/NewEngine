use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{atomic::Ordering, Arc, Mutex, OnceLock};

use super::super::state::{ctx, GatewayRegistryCache};

thread_local! {
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

static GATEWAY_RESOLUTION_DIAGNOSTICS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub(super) fn should_emit_gateway_resolution(gateway_id: &str, resolution: &str) -> bool {
    let state = GATEWAY_RESOLUTION_DIAGNOSTICS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut state = match state.lock() {
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

fn build_gateway_registry_snapshot() -> crate::service_gateway::ActiveGatewayRegistry {
    let c = ctx();

    let services = {
        let services = match c.services.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
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
    };

    let plugin_origins = {
        let origins = match c.plugin_origins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        origins.clone()
    };

    let descriptors = {
        let descriptors = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        descriptors
            .iter()
            .map(|(plugin_id, descriptor)| {
                crate::service_gateway::PluginDescriptorFact::new(
                    plugin_id.clone(),
                    descriptor.clone(),
                    plugin_origins
                        .get(plugin_id)
                        .copied()
                        .unwrap_or(crate::service_gateway::GatewayProviderOrigin::GamePlugin),
                )
            })
            .collect::<Vec<_>>()
    };

    let gateway_provider_routes = {
        let gateways = match c.gateway_provider_routes.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
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
                    [
                        newengine_service_api::system_tag::ENGINE_DOMAIN,
                        newengine_service_api::system_tag::PROVIDER_BACKEND,
                    ],
                )
            })
            .collect::<Vec<_>>()
    };

    let selection_policies = {
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
    };

    crate::service_gateway::ActiveGatewayRegistry::from_facts_with_policy(
        &descriptors,
        &services,
        &gateway_provider_routes,
        &selection_policies,
    )
}

pub(super) fn gateway_registry_snapshot() -> Arc<crate::service_gateway::ActiveGatewayRegistry> {
    let c = ctx();

    loop {
        let generation_before = c.services_generation.load(Ordering::Acquire);

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

        if generation_before != generation_after {
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

pub(crate) fn active_engine_gateways() -> Vec<String> {
    gateway_registry_snapshot().gateway_ids()
}
