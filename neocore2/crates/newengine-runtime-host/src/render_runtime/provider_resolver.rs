use newengine_plugin_api::{CapabilityKind, CapabilityRole};
use newengine_plugin_host::{PluginSnapshotEntry, PluginsSnapshot};
use newengine_render_api::{RenderBackendInfo, RENDER_SERVICE_ID};

use crate::render_runtime::types::RENDER_BACKEND_CAPABILITY_ID;

#[derive(Clone, Debug)]
pub(crate) struct RenderBackendSelection {
    pub(crate) provider_plugin_id: String,
    pub(crate) provider_state: String,
    pub(crate) matched_by: &'static str,
}

pub(crate) struct RenderProviderResolver;

impl RenderProviderResolver {
    pub(crate) fn resolve(
        snapshot: Option<&PluginsSnapshot>,
        info: &RenderBackendInfo,
    ) -> Result<RenderBackendSelection, String> {
        let Some(snapshot) = snapshot else {
            return Err(format!(
                "render provider snapshot is unavailable while binding backend '{}'; render backends must be validated as service plugins with '{}' and '{}'",
                info.backend_id,
                RENDER_BACKEND_CAPABILITY_ID,
                RENDER_SERVICE_ID,
            ));
        };

        let provider = snapshot.plugins.iter().find(|plugin| {
            plugin.id == info.backend_id
                && plugin_declares_render_backend(plugin)
                && plugin_declares_service(plugin, RENDER_SERVICE_ID)
        });

        if let Some(plugin) = provider {
            return Ok(RenderBackendSelection {
                provider_plugin_id: plugin.id.clone(),
                provider_state: plugin.state.clone(),
                matched_by: "plugin-id+capability:render.backend+service:render.api",
            });
        }

        let render_plugins: Vec<String> = snapshot
            .plugins
            .iter()
            .filter(|plugin| {
                plugin_declares_render_backend(plugin)
                    || plugin_declares_service(plugin, RENDER_SERVICE_ID)
            })
            .map(|plugin| {
                let has_backend_cap = plugin_declares_render_backend(plugin);
                let has_service = plugin_declares_service(plugin, RENDER_SERVICE_ID);
                format!(
                    "{}:{} backend_cap={} service={}",
                    plugin.id, plugin.state, has_backend_cap, has_service
                )
            })
            .collect();

        Err(format!(
            "active render service backend '{}' is not backed by a loaded plugin declaring both '{}' and '{}'; available render providers=[{}]",
            info.backend_id,
            RENDER_BACKEND_CAPABILITY_ID,
            RENDER_SERVICE_ID,
            render_plugins.join(", "),
        ))
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
pub(crate) fn plugin_declares_render_backend(plugin: &PluginSnapshotEntry) -> bool {
    plugin.capabilities.iter().any(|cap| {
        cap.role == CapabilityRole::Provides
            && cap.id.as_str() == RENDER_BACKEND_CAPABILITY_ID
    })
}
