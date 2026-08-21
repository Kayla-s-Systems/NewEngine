use super::super::state::{
    bump_services_generation, ctx, EngineGatewayRouteSnapshot, GatewayProviderRouteEntry,
};
use super::registry::{
    emit_gateway_route_missing, emit_gateway_route_selected, emit_gateway_route_shadowed,
    gateway_registry_snapshot, should_emit_gateway_resolution,
};

/// Resolve a host-owned facade service gateway to the active registered provider
/// service declared by plugin metadata.
///
/// The lookup is purely descriptor-driven: if no loaded provider declares the
/// requested gateway id, resolution returns `None`. It does not branch on
/// concrete domains such as assets/render/physics/input.
pub fn resolve_service_for_engine_gateway(gateway_id: &str) -> Option<String> {
    let registry = gateway_registry_snapshot();
    let route = registry.resolve_route(gateway_id);
    match route {
        Some(route) => {
            let resolution = format!(
                "selected:{}:{}:{}:{}",
                route.provider_service_id,
                route.provider_route_id.as_deref().unwrap_or(""),
                route.provider_owner_id,
                route.active_score
            );
            if should_emit_gateway_resolution(gateway_id, &resolution) {
                emit_gateway_route_selected(gateway_id, route);
            }
            Some(route.provider_service_id.clone())
        }
        None => {
            if should_emit_gateway_resolution(gateway_id, "missing") {
                emit_gateway_route_missing(gateway_id);
            }
            None
        }
    }
}

pub fn engine_gateway_has_capability(gateway_id: &str, capability_id: &str) -> bool {
    gateway_registry_snapshot().has_gateway_capability(gateway_id, capability_id)
}

fn route_snapshot(
    route: &crate::service_gateway::ActiveGatewayRoute,
    active: bool,
    selection_state: impl Into<String>,
    selection_reason: impl Into<String>,
) -> EngineGatewayRouteSnapshot {
    EngineGatewayRouteSnapshot {
        gateway_id: route.gateway_id.clone(),
        service_kind: route.service_kind.as_str().to_owned(),
        provider_service_id: route.provider_service_id.clone(),
        provider_route_id: route.provider_route_id.clone(),
        provider_abi: route.provider_abi.clone(),
        provider_owner_id: route.provider_owner_id.clone(),
        backend_capability_id: route.backend_capability_id.clone(),
        backend_priority: route.backend_priority,
        origin: route.origin.as_str().to_owned(),
        override_mode: route.override_mode.as_str().to_owned(),
        active_score: route.active_score,
        active,
        selection_state: selection_state.into(),
        selection_reason: selection_reason.into(),
    }
}

#[inline]
fn active_selection_reason(route: &crate::service_gateway::ActiveGatewayRoute) -> String {
    format!(
        "selected_by_registry score={} origin='{}' priority={}",
        route.active_score,
        route.origin.as_str(),
        route.backend_priority
    )
}

pub fn list_engine_gateway_routes() -> Vec<EngineGatewayRouteSnapshot> {
    let registry = gateway_registry_snapshot();
    registry
        .routes()
        .iter()
        .map(|route| {
            let active_route = registry.resolve_route(&route.gateway_id);
            if let Some(active_route) = active_route {
                if active_route.provider_service_id != route.provider_service_id
                    || active_route.provider_owner_id != route.provider_owner_id
                {
                    emit_gateway_route_shadowed(route, active_route);
                }
            }
            let active = match active_route {
                Some(active_route) => {
                    active_route.provider_service_id == route.provider_service_id
                        && active_route.provider_owner_id == route.provider_owner_id
                }
                None => false,
            };
            if active {
                route_snapshot(route, true, "active", active_selection_reason(route))
            } else if let Some(active_route) = active_route {
                route_snapshot(
                    route,
                    false,
                    "shadowed",
                    format!(
                        "shadowed_by service='{}' provider_route='{}' owner='{}' score={}",
                        active_route.provider_service_id,
                        active_route
                            .provider_route_id
                            .as_deref()
                            .unwrap_or("<provider-route-unset>"),
                        active_route.provider_owner_id,
                        active_route.active_score
                    ),
                )
            } else {
                route_snapshot(
                    route,
                    false,
                    "unavailable",
                    "no active route for gateway after registry resolution",
                )
            }
        })
        .collect()
}

pub fn active_engine_gateway_route(gateway_id: &str) -> Option<EngineGatewayRouteSnapshot> {
    gateway_registry_snapshot()
        .resolve_route(gateway_id)
        .map(|route| route_snapshot(route, true, "active", active_selection_reason(route)))
}

#[allow(clippy::too_many_arguments)]
fn register_engine_gateway_provider_route_with_origin<S>(
    gateway_id: &str,
    service_kind: S,
    provider_service_id: &str,
    provider_route_id: &str,
    provider_abi: Option<&str>,
    backend_capability_id: &str,
    backend_priority: i32,
    provider_owner_id: &str,
    origin: crate::service_gateway::GatewayProviderOrigin,
) -> Result<(), String>
where
    S: AsRef<str>,
{
    if !newengine_service_api::is_engine_service_gateway_id(gateway_id) {
        return Err(format!(
            "engine-runtime route id must start with 'engine.': {gateway_id}"
        ));
    }
    let raw_service_kind = service_kind.as_ref();
    let Some(service_kind) = newengine_service_api::normalize_service_kind(raw_service_kind) else {
        return Err(format!(
            "engine-runtime route service_kind is invalid: '{}'",
            raw_service_kind
        ));
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
        return Err(format!(
            "engine-runtime route provider_route_id must start with 'engine.': {provider_route_id}"
        ));
    }
    if !crate::service_gateway::provider_route_extends_gateway_parent(gateway_id, provider_route_id)
    {
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
            provider_abi: provider_abi
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
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
    newengine_ulog_api::ulog::info!(
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
        None,
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
        None,
        backend_capability_id,
        -10_000,
        provider_owner_id,
        crate::service_gateway::GatewayProviderOrigin::NullProvider,
    )
}

pub fn register_null_engine_gateway_provider_route_with_abi<S>(
    gateway_id: &str,
    service_kind: S,
    provider_service_id: &str,
    provider_route_id: &str,
    provider_abi: &str,
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
        Some(provider_abi),
        backend_capability_id,
        -10_000,
        provider_owner_id,
        crate::service_gateway::GatewayProviderOrigin::NullProvider,
    )
}
