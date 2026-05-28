use newengine_core::ModuleCtx;
use newengine_plugin_api::{CapabilityKind, CapabilityRole};
use newengine_plugin_host::{PluginSnapshotEntry, PluginsSnapshot};
use newengine_service_api::BackendServiceSpec;

use super::BackendSelection;

pub(crate) fn resolve_backend_provider(
    snapshot: Option<&PluginsSnapshot>,
    spec: BackendServiceSpec,
) -> Result<BackendSelection, String> {
    let Some(route) = newengine_plugin_host::active_engine_gateway_route(spec.engine_gateway_id) else {
        let candidates = snapshot
            .map(|snapshot| service_backend_candidates(snapshot, spec).join(", "))
            .unwrap_or_else(|| "<plugin snapshot unavailable>".to_owned());
        return Err(format!(
            "active {} gateway route '{}' is unavailable; provider selection must be resolved through ActiveGatewayRegistry, not backend_id; available {} providers=[{}]",
            spec.domain,
            spec.engine_gateway_id,
            spec.domain,
            candidates,
        ));
    };

    if route.backend_capability_id != spec.backend_capability_id {
        return Err(format!(
            "active {} gateway route '{}' selected provider='{}' service='{}' with capability='{}', expected capability='{}'",
            spec.domain,
            spec.engine_gateway_id,
            route.provider_owner_id,
            route.provider_service_id,
            route.backend_capability_id,
            spec.backend_capability_id,
        ));
    }

    if route.provider_service_id.trim().is_empty() {
        return Err(format!(
            "active {} gateway route '{}' has empty provider service id",
            spec.domain, spec.engine_gateway_id
        ));
    }

    if route.origin == "engine-runtime" || route.origin == "null-provider" {
        return Ok(BackendSelection {
            provider_plugin_id: route.provider_owner_id.clone(),
            provider_state: route.origin.clone(),
            matched_by: format!(
                "active-gateway-route:{}->{} capability:{} score:{}",
                spec.engine_gateway_id,
                route.provider_service_id,
                route.backend_capability_id,
                route.active_score,
            ),
        });
    }

    let Some(snapshot) = snapshot else {
        return Err(format!(
            "active {} gateway route '{}' selected provider='{}' service='{}', but plugin snapshot is unavailable for provider-state validation",
            spec.domain,
            spec.engine_gateway_id,
            route.provider_owner_id,
            route.provider_service_id,
        ));
    };

    let Some(plugin) = snapshot.plugins.iter().find(|plugin| plugin.id == route.provider_owner_id) else {
        let candidates = service_backend_candidates(snapshot, spec);
        return Err(format!(
            "active {} gateway route '{}' selected owner='{}' service='{}', but that plugin is not loaded; available {} providers=[{}]",
            spec.domain,
            spec.engine_gateway_id,
            route.provider_owner_id,
            route.provider_service_id,
            spec.domain,
            candidates.join(", "),
        ));
    };

    let has_backend_cap = plugin_declares_backend_capability(plugin, spec.backend_capability_id);
    let has_routed_service = plugin_declares_service(plugin, &route.provider_service_id);
    if !has_backend_cap || !has_routed_service {
        return Err(format!(
            "active {} gateway route '{}' selected plugin='{}' service='{}', but snapshot validation failed backend_cap={} routed_service={}; provider must declare the routed service contract and '{}', backend_id is diagnostic only",
            spec.domain,
            spec.engine_gateway_id,
            plugin.id,
            route.provider_service_id,
            has_backend_cap,
            has_routed_service,
            spec.backend_capability_id,
        ));
    }

    Ok(BackendSelection {
        provider_plugin_id: plugin.id.clone(),
        provider_state: plugin.state.clone(),
        matched_by: format!(
            "active-gateway-route:{}->{} owner:{} capability:{} score:{}",
            spec.engine_gateway_id,
            route.provider_service_id,
            route.provider_owner_id,
            route.backend_capability_id,
            route.active_score,
        ),
    })
}

pub(crate) fn explain_backend_unavailability<E: Send + 'static>(
    ctx: &ModuleCtx<'_, E>,
    spec: BackendServiceSpec,
    service_error: &str,
) -> String {
    let Some(snapshot) = ctx.resources().get::<PluginsSnapshot>() else {
        return format!(
            "{} service '{}' is unavailable: {}",
            spec.domain, spec.engine_gateway_id, service_error
        );
    };

    let loaded_plugins: Vec<String> = snapshot
        .plugins
        .iter()
        .filter(|plugin| {
            plugin_declares_backend_capability(plugin, spec.backend_capability_id)
                || plugin_declares_service_for_spec(plugin, spec)
        })
        .map(|plugin| format!("{}:{}", plugin.id, plugin.state))
        .collect();

    if loaded_plugins.is_empty() {
        format!(
            "no {} backend plugin was loaded; service '{}' is unavailable: {}",
            spec.domain, spec.engine_gateway_id, service_error
        )
    } else {
        format!(
            "loaded {} providers=[{}], but service '{}' is unavailable: {}",
            spec.domain,
            loaded_plugins.join(", "),
            spec.engine_gateway_id,
            service_error
        )
    }
}

#[inline]
pub(crate) fn plugin_declares_service(plugin: &PluginSnapshotEntry, service_id: &str) -> bool {
    plugin.capabilities.iter().any(|cap| {
        cap.role == CapabilityRole::Provides
            && cap.kind == CapabilityKind::ServiceV1
            && cap.id.as_str() == service_id
    })
}

#[inline]
pub(crate) fn plugin_declares_service_for_spec(
    plugin: &PluginSnapshotEntry,
    spec: BackendServiceSpec,
) -> bool {
    if plugin_declares_service(plugin, spec.provider_service_id) {
        return true;
    }

    plugin.capabilities.iter().any(|cap| {
        if cap.role != CapabilityRole::Provides || cap.id.as_str() != spec.backend_capability_id {
            return false;
        }
        serde_json::from_str::<serde_json::Value>(cap.describe_json.as_str())
            .ok()
            .and_then(|value| value.get("contract").and_then(|v| v.as_str()).map(str::to_owned))
            .is_some_and(|service_id| plugin_declares_service(plugin, &service_id))
    })
}

#[inline]
pub(crate) fn plugin_declares_backend_capability(
    plugin: &PluginSnapshotEntry,
    backend_capability_id: &str,
) -> bool {
    plugin.capabilities.iter().any(|cap| {
        cap.role == CapabilityRole::Provides && cap.id.as_str() == backend_capability_id
    })
}

fn service_backend_candidates(snapshot: &PluginsSnapshot, spec: BackendServiceSpec) -> Vec<String> {
    snapshot
        .plugins
        .iter()
        .filter(|plugin| {
            plugin_declares_backend_capability(plugin, spec.backend_capability_id)
                || plugin_declares_service_for_spec(plugin, spec)
        })
        .map(|plugin| {
            let has_backend_cap = plugin_declares_backend_capability(plugin, spec.backend_capability_id);
            let has_service = plugin_declares_service_for_spec(plugin, spec);
            format!(
                "{}:{} backend_cap={} service={}",
                plugin.id, plugin.state, has_backend_cap, has_service
            )
        })
        .collect()
}
