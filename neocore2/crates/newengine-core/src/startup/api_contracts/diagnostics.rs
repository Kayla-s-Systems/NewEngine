#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_host::PluginSnapshotEntry;

pub(crate) fn provider_has_required_capability(
    service_id: &str,
    capability_id: &str,
    plugins: &[PluginSnapshotEntry],
) -> bool {
    if newengine_plugin_host::engine_gateway_has_capability(service_id, capability_id) {
        return true;
    }

    if service_description_has_capability(service_id, capability_id) {
        return true;
    }

    plugins.iter().any(|plugin| {
        let has_service = plugin.capabilities.iter().any(|cap| {
            cap.role == newengine_plugin_api::CapabilityRole::Provides
                && cap.kind == newengine_plugin_api::CapabilityKind::ServiceV1
                && cap.id.as_str() == service_id
        });
        let has_capability = plugin.capabilities.iter().any(|cap| {
            cap.role == newengine_plugin_api::CapabilityRole::Provides
                && cap.id.as_str() == capability_id
        });
        has_service && has_capability
    })
}

pub(crate) fn service_origin(service_id: &str) -> Option<String> {
    let description = newengine_plugin_host::describe_service(service_id)?;
    let json = serde_json::from_str::<serde_json::Value>(&description).ok()?;
    json.get("origin")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

fn service_description_has_capability(service_id: &str, capability_id: &str) -> bool {
    let Some(description) = newengine_plugin_host::describe_service(service_id) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&description) else {
        return false;
    };

    if json.get("capability").and_then(|value| value.as_str()) == Some(capability_id) {
        return true;
    }

    json.get("capabilities")
        .and_then(|value| value.as_array())
        .map(|items| {
            items.iter().any(|item| {
                item.as_str() == Some(capability_id)
                    || item.get("id").and_then(|value| value.as_str()) == Some(capability_id)
            })
        })
        .unwrap_or(false)
}

pub(crate) fn provider_for(plugins: &[PluginSnapshotEntry], service_id: &str) -> String {
    if let Some(route) = newengine_plugin_host::active_engine_gateway_route(service_id) {
        return format!(
            "{}:{} service={} capability={} mode={} priority={} score={}",
            route.origin,
            route.provider_owner_id,
            route.provider_service_id,
            route.backend_capability_id,
            route.override_mode,
            route.backend_priority,
            route.active_score
        );
    }

    let mut providers = plugins
        .iter()
        .filter(|plugin| declares_service_or_gateway(plugin, service_id))
        .map(|plugin| {
            format!(
                "{}@{} state={} path={}",
                plugin.id,
                plugin.version,
                plugin.state,
                plugin.path.display()
            )
        })
        .collect::<Vec<_>>();

    providers.sort();
    if providers.is_empty() {
        service_description_provider(service_id).unwrap_or_else(|| "<unknown>".to_owned())
    } else {
        providers.join("; ")
    }
}

fn service_description_provider(service_id: &str) -> Option<String> {
    let description = newengine_plugin_host::describe_service(service_id)?;
    let json = serde_json::from_str::<serde_json::Value>(&description).ok()?;
    let origin = json
        .get("origin")
        .and_then(|value| value.as_str())
        .unwrap_or("direct");
    let owner = json
        .get("owner")
        .and_then(|value| value.as_str())
        .unwrap_or("<unknown-owner>");
    let capability = json
        .get("capability")
        .and_then(|value| value.as_str())
        .unwrap_or("-");
    Some(format!(
        "{}:{} service={} capability={}",
        origin, owner, service_id, capability
    ))
}

fn declares_service_or_gateway(plugin: &PluginSnapshotEntry, service_id: &str) -> bool {
    plugin.capabilities.iter().any(|cap| {
        cap.role == newengine_plugin_api::CapabilityRole::Provides
            && cap.kind == newengine_plugin_api::CapabilityKind::ServiceV1
            && cap.id.as_str() == service_id
    }) || plugin
        .capabilities
        .iter()
        .any(|cap| capability_engine_gateway(cap).as_deref() == Some(service_id))
}

fn capability_engine_gateway(capability: &newengine_plugin_api::CapabilityDesc) -> Option<String> {
    if capability.role != newengine_plugin_api::CapabilityRole::Provides {
        return None;
    }
    match capability.to_v2_compat().route {
        abi_stable::std_types::ROption::RSome(route) => Some(route.engine_gateway.to_string()),
        abi_stable::std_types::ROption::RNone => None,
    }
}
