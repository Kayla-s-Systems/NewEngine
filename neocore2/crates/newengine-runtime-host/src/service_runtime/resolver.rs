use newengine_core::ModuleCtx;
use newengine_plugin_api::{CapabilityKind, CapabilityRole};
use newengine_plugin_host::{PluginSnapshotEntry, PluginsSnapshot};
use newengine_service_api::BackendServiceSpec;

use super::BackendSelection;

pub(crate) fn resolve_backend_provider(
    snapshot: Option<&PluginsSnapshot>,
    spec: BackendServiceSpec,
    backend_id: &str,
) -> Result<BackendSelection, String> {
    let Some(snapshot) = snapshot else {
        return Err(format!(
            "{} provider snapshot is unavailable while binding backend '{}'; {} backends must be validated as service plugins with '{}' and '{}'",
            spec.domain,
            backend_id,
            spec.domain,
            spec.backend_capability_id,
            spec.service_id,
        ));
    };

    let provider = snapshot.plugins.iter().find(|plugin| {
        plugin.id == backend_id
            && plugin_declares_backend_capability(plugin, spec.backend_capability_id)
            && plugin_declares_service(plugin, spec.service_id)
    });

    if let Some(plugin) = provider {
        return Ok(BackendSelection {
            provider_plugin_id: plugin.id.clone(),
            provider_state: plugin.state.clone(),
            matched_by: format!(
                "plugin-id+capability:{}+service:{}",
                spec.backend_capability_id, spec.service_id
            ),
        });
    }

    let candidates = service_backend_candidates(snapshot, spec);
    Err(format!(
        "active {} service backend '{}' is not backed by a loaded plugin declaring both '{}' and '{}'; available {} providers=[{}]",
        spec.domain,
        backend_id,
        spec.backend_capability_id,
        spec.service_id,
        spec.domain,
        candidates.join(", "),
    ))
}

pub(crate) fn explain_backend_unavailability<E: Send + 'static>(
    ctx: &ModuleCtx<'_, E>,
    spec: BackendServiceSpec,
    service_error: &str,
) -> String {
    let Some(snapshot) = ctx.resources().get::<PluginsSnapshot>() else {
        return format!(
            "{} service '{}' is unavailable: {}",
            spec.domain, spec.service_id, service_error
        );
    };

    let loaded_plugins: Vec<String> = snapshot
        .plugins
        .iter()
        .filter(|plugin| {
            plugin_declares_backend_capability(plugin, spec.backend_capability_id)
                || plugin_declares_service(plugin, spec.service_id)
        })
        .map(|plugin| format!("{}:{}", plugin.id, plugin.state))
        .collect();

    if loaded_plugins.is_empty() {
        format!(
            "no {} backend plugin was loaded; service '{}' is unavailable: {}",
            spec.domain, spec.service_id, service_error
        )
    } else {
        format!(
            "loaded {} providers=[{}], but service '{}' is unavailable: {}",
            spec.domain,
            loaded_plugins.join(", "),
            spec.service_id,
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
                || plugin_declares_service(plugin, spec.service_id)
        })
        .map(|plugin| {
            let has_backend_cap = plugin_declares_backend_capability(plugin, spec.backend_capability_id);
            let has_service = plugin_declares_service(plugin, spec.service_id);
            format!(
                "{}:{} backend_cap={} service={}",
                plugin.id, plugin.state, has_backend_cap, has_service
            )
        })
        .collect()
}
