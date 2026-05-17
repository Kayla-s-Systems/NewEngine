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

pub(crate) fn provider_for(plugins: &[PluginSnapshotEntry], service_id: &str) -> String {
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
        "<unknown>".to_owned()
    } else {
        providers.join("; ")
    }
}

fn declares_service_or_gateway(plugin: &PluginSnapshotEntry, service_id: &str) -> bool {
    plugin.capabilities.iter().any(|cap| {
        cap.role == newengine_plugin_api::CapabilityRole::Provides
            && cap.kind == newengine_plugin_api::CapabilityKind::ServiceV1
            && cap.id.as_str() == service_id
    }) || plugin.capabilities.iter().any(|cap| capability_engine_gateway(cap).as_deref() == Some(service_id))
}

fn capability_engine_gateway(capability: &newengine_plugin_api::CapabilityDesc) -> Option<String> {
    if capability.role != newengine_plugin_api::CapabilityRole::Provides {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(capability.describe_json.as_str())
        .ok()
        .and_then(|value| {
            value
                .get("engine_gateway")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
}
