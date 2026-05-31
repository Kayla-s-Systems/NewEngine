use newengine_plugin_api::{CapabilityKind, CapabilityRole};
use std::sync::atomic::Ordering;

use super::state::{ctx, bump_services_generation, EngineGatewayRouteSnapshot, GatewayProviderRouteEntry, GatewayRegistryCache};

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

    crate::service_gateway::ActiveGatewayRegistry::from_facts(
        &descriptors,
        &services,
        &gateway_provider_routes,
    )
}

fn gateway_registry_snapshot() -> crate::service_gateway::ActiveGatewayRegistry {
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
                    return cache.registry.clone();
                }
            }
        }

        let registry = build_gateway_registry_snapshot();
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
                registry: registry.clone(),
            });
        }

        return registry;
    }
}


pub(crate) fn active_engine_gateways() -> Vec<String> {
    gateway_registry_snapshot().gateway_ids()
}

/// Resolve a host-owned facade service gateway to the active registered provider
/// service declared by plugin metadata.
///
/// The lookup is purely descriptor-driven: if no loaded provider declares the
/// requested gateway id, resolution returns `None`. It does not branch on
/// concrete domains such as assets/render/physics/input.
pub fn resolve_service_for_engine_gateway(gateway_id: &str) -> Option<String> {
    gateway_registry_snapshot().resolve_gateway(gateway_id)
}

pub fn engine_gateway_has_capability(gateway_id: &str, capability_id: &str) -> bool {
    gateway_registry_snapshot().has_gateway_capability(gateway_id, capability_id)
}


pub fn list_engine_gateway_routes() -> Vec<EngineGatewayRouteSnapshot> {
    let registry = gateway_registry_snapshot();
    registry
        .routes()
        .iter()
        .map(|route| {
            let active_route = registry.resolve_route(&route.gateway_id);
            let active = match active_route {
                Some(active_route) => {
                    active_route.provider_service_id == route.provider_service_id
                        && active_route.provider_owner_id == route.provider_owner_id
                }
                None => false,
            };
            let (selection_state, selection_reason) = if active {
                (
                    "active".to_owned(),
                    format!(
                        "selected_by_registry score={} origin='{}' priority={}",
                        route.active_score,
                        route.origin.as_str(),
                        route.backend_priority
                    ),
                )
            } else if let Some(active_route) = active_route {
                (
                    "shadowed".to_owned(),
                    format!(
                        "shadowed_by service='{}' provider_route='{}' owner='{}' score={}",
                        active_route.provider_service_id,
                        active_route.provider_route_id.as_deref().unwrap_or("<provider-route-unset>"),
                        active_route.provider_owner_id,
                        active_route.active_score
                    ),
                )
            } else {
                (
                    "unavailable".to_owned(),
                    "no active route for gateway after registry resolution".to_owned(),
                )
            };
            let override_mode: crate::service_gateway::GatewayOverrideMode = route.override_mode;
            EngineGatewayRouteSnapshot {
                gateway_id: route.gateway_id.clone(),
                service_kind: route.service_kind.as_str().to_owned(),
                provider_service_id: route.provider_service_id.clone(),
                provider_route_id: route.provider_route_id.clone(),
                provider_owner_id: route.provider_owner_id.clone(),
                backend_capability_id: route.backend_capability_id.clone(),
                backend_priority: route.backend_priority,
                origin: route.origin.as_str().to_owned(),
                override_mode: override_mode.as_str().to_owned(),
                active_score: route.active_score,
                active,
                selection_state,
                selection_reason,
            }
        })
        .collect()
}

pub fn active_engine_gateway_route(gateway_id: &str) -> Option<EngineGatewayRouteSnapshot> {
    gateway_registry_snapshot()
        .resolve_route(gateway_id)
        .map(|route| {
            let override_mode: crate::service_gateway::GatewayOverrideMode = route.override_mode;
            EngineGatewayRouteSnapshot {
                gateway_id: route.gateway_id.clone(),
                service_kind: route.service_kind.as_str().to_owned(),
                provider_service_id: route.provider_service_id.clone(),
                provider_route_id: route.provider_route_id.clone(),
                provider_owner_id: route.provider_owner_id.clone(),
                backend_capability_id: route.backend_capability_id.clone(),
                backend_priority: route.backend_priority,
                origin: route.origin.as_str().to_owned(),
                override_mode: override_mode.as_str().to_owned(),
                active_score: route.active_score,
                active: true,
                selection_state: "active".to_owned(),
                selection_reason: format!(
                    "selected_by_registry score={} origin='{}' priority={}",
                    route.active_score,
                    route.origin.as_str(),
                    route.backend_priority
                ),
            }
        })
}

fn register_engine_gateway_provider_route_with_origin<S>(
    gateway_id: &str,
    service_kind: S,
    provider_service_id: &str,
    provider_route_id: &str,
    backend_capability_id: &str,
    backend_priority: i32,
    provider_owner_id: &str,
    origin: crate::service_gateway::GatewayProviderOrigin,
) -> Result<(), String>
where
    S: AsRef<str>,
{
    if !newengine_service_api::is_engine_service_gateway_id(gateway_id) {
        return Err(format!("engine-runtime route id must start with 'engine.': {gateway_id}"));
    }
    let raw_service_kind = service_kind.as_ref();
    let Some(service_kind) = newengine_service_api::normalize_service_kind(raw_service_kind) else {
        return Err(format!("engine-runtime route service_kind is invalid: '{}'", raw_service_kind));
    };
    if !newengine_service_api::engine_gateway_matches_service_kind(gateway_id, &service_kind) {
        return Err(format!(
            "engine-runtime route service_kind/domain mismatch: gateway='{gateway_id}' service_kind='{service_kind}' expected='{}'",
            newengine_service_api::service_kind_from_engine_gateway_id(gateway_id).unwrap_or_else(|| "<invalid>".to_owned())
        ));
    }
    if provider_service_id.trim().is_empty() {
        return Err("engine-runtime route provider_service_id is empty".to_owned());
    }
    if provider_route_id.trim().is_empty() {
        return Err("engine-runtime route provider_route_id is empty".to_owned());
    }
    if !newengine_service_api::is_engine_service_gateway_id(provider_route_id) {
        return Err(format!("engine-runtime route provider_route_id must start with 'engine.': {provider_route_id}"));
    }
    if !provider_route_extends_gateway_parent(gateway_id, provider_route_id) {
        return Err(format!(
            "engine-runtime route must extend its gateway root/provider namespace: gateway='{gateway_id}' provider_route='{provider_route_id}'"
        ));
    }
    if backend_capability_id.trim().is_empty() {
        return Err("engine-runtime route backend_capability_id is empty".to_owned());
    }

    let c = ctx();
    {
        let services = match c.services.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        match services.get(provider_service_id) {
            Some(entry) if entry.owner_plugin_id.is_none() => {}
            Some(entry) => {
                return Err(format!(
                    "engine-runtime route '{}' cannot route to plugin-owned service '{}' owner='{}'",
                    gateway_id,
                    provider_service_id,
                    entry.owner_plugin_id.as_deref().unwrap_or("<unknown>")
                ));
            }
            None => {
                return Err(format!(
                    "engine-runtime route '{}' cannot route to unregistered service '{}'",
                    gateway_id, provider_service_id
                ));
            }
        }
    }

    let key = format!("{}::{}", gateway_id, provider_service_id);
    let mut gateways = match c.gateway_provider_routes.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    gateways.insert(
        key,
        GatewayProviderRouteEntry {
            gateway_id: gateway_id.to_owned(),
            service_kind: service_kind.clone(),
            provider_service_id: provider_service_id.to_owned(),
            provider_route_id: provider_route_id.to_owned(),
            provider_owner_id: if provider_owner_id.trim().is_empty() {
                "engine".to_owned()
            } else {
                provider_owner_id.to_owned()
            },
            backend_capability_id: backend_capability_id.to_owned(),
            backend_priority,
            origin,
        },
    );

    bump_services_generation();
    log::info!(
        "gateways: registered provider route gateway='{}' service='{}' provider_route='{}' kind='{}' capability='{}' priority={} owner='{}' origin='{}'",
        gateway_id,
        provider_service_id,
        provider_route_id,
        service_kind,
        backend_capability_id,
        backend_priority,
        provider_owner_id,
        origin.as_str(),
    );
    Ok(())
}

pub fn register_engine_gateway_provider_route<S>(
    gateway_id: &str,
    service_kind: S,
    provider_service_id: &str,
    provider_route_id: &str,
    backend_capability_id: &str,
    backend_priority: i32,
    provider_owner_id: &str,
) -> Result<(), String>
where
    S: AsRef<str>,
{
    register_engine_gateway_provider_route_with_origin(
        gateway_id,
        service_kind,
        provider_service_id,
        provider_route_id,
        backend_capability_id,
        backend_priority,
        provider_owner_id,
        crate::service_gateway::GatewayProviderOrigin::EngineRuntime,
    )
}

pub fn register_null_engine_gateway_provider_route<S>(
    gateway_id: &str,
    service_kind: S,
    provider_service_id: &str,
    provider_route_id: &str,
    backend_capability_id: &str,
    provider_owner_id: &str,
) -> Result<(), String>
where
    S: AsRef<str>,
{
    register_engine_gateway_provider_route_with_origin(
        gateway_id,
        service_kind,
        provider_service_id,
        provider_route_id,
        backend_capability_id,
        -10_000,
        provider_owner_id,
        crate::service_gateway::GatewayProviderOrigin::NullProvider,
    )
}

#[inline]
fn provider_route_extends_gateway_parent(gateway_id: &str, provider_route_id: &str) -> bool {
    if provider_route_id == gateway_id {
        return false;
    }
    let gateway_parts = gateway_id.split('.').collect::<Vec<_>>();
    let provider_parts = provider_route_id.split('.').collect::<Vec<_>>();
    if gateway_parts.len() < 2 || provider_parts.len() <= gateway_parts.len() {
        return false;
    }
    if gateway_parts.get(0) != Some(&"engine") || provider_parts.get(0) != Some(&"engine") {
        return false;
    }
    if gateway_parts.get(1) != provider_parts.get(1) {
        return false;
    }

    // Root gateway implementation: engine.camera -> engine.camera.stargazer.
    if gateway_parts.len() == 2 {
        return provider_parts.starts_with(&gateway_parts);
    }

    // Child gateway implementation: engine.assets.textures -> engine.assets.starvault.textures.
    // The provider name sits immediately below the root gateway, and the remaining
    // tail must extend the child API instead of inventing another top-level island.
    let child_tail = &gateway_parts[2..];
    let Some(provider_tail) = provider_parts.get(3..) else {
        return false;
    };
    provider_tail.starts_with(child_tail)
}

#[inline]
fn parse_backend_priority(json: &str) -> i64 {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("backend_priority").and_then(|x| x.as_i64()))
        .unwrap_or(0)
}

/// Resolve the active registered provider service for a backend capability.
///
/// This is the host-owned service gateway primitive: callers ask the engine for
/// a domain service, while the host selects the concrete provider service from
/// descriptor facts instead of requiring consumers to know provider ids.
pub fn resolve_service_for_backend_capability(capability_id: &str) -> Option<String> {
    let c = ctx();
    let services = match c.services.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let descriptors = match c.plugin_descriptors.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let plugin_origins = match c.plugin_origins.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    let mut candidates: Vec<(i64, i64, String, String)> = Vec::new();

    for (service_id, entry) in services.iter() {
        let Some(owner) = entry.owner_plugin_id.as_deref() else {
            continue;
        };
        let Some(descriptor) = descriptors.get(owner) else {
            continue;
        };

        let Some(backend_capability) = descriptor.capabilities.iter().find(|cap| {
            cap.role == CapabilityRole::Provides && cap.id.as_str() == capability_id
        }) else {
            continue;
        };

        let declares_registered_service = descriptor.capabilities.iter().any(|cap| {
            cap.role == CapabilityRole::Provides
                && cap.kind == CapabilityKind::ServiceV1
                && cap.id.as_str() == service_id
        });
        if !declares_registered_service {
            continue;
        }

        let backend_priority = parse_backend_priority(backend_capability.describe_json.as_str());
        let origin = plugin_origins
            .get(owner)
            .copied()
            .unwrap_or(crate::service_gateway::GatewayProviderOrigin::GamePlugin);
        candidates.push((
            origin.origin_bias() + backend_priority,
            backend_priority,
            service_id.clone(),
            owner.to_owned(),
        ));
    }

    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.3.cmp(&b.3))
    });

    candidates.into_iter().map(|(_, _, service_id, _)| service_id).next()
}
