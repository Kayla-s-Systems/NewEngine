use newengine_plugin_api::{CapabilityKind, CapabilityRole};
use newengine_plugin_host::{PluginSnapshotEntry, PluginsSnapshot};
use newengine_physics_api::{PhysicsBackendInfo, PHYSICS_SERVICE_ID};

use crate::physics_runtime::types::PHYSICS_BACKEND_CAPABILITY_ID;

#[derive(Clone, Debug)]
pub(crate) struct PhysicsBackendSelection {
    pub(crate) provider_plugin_id: String,
    pub(crate) provider_state: String,
    pub(crate) matched_by: &'static str,
}

pub(crate) struct PhysicsProviderResolver;

impl PhysicsProviderResolver {
    pub(crate) fn resolve(
        snapshot: Option<&PluginsSnapshot>,
        info: &PhysicsBackendInfo,
    ) -> Result<PhysicsBackendSelection, String> {
        let Some(snapshot) = snapshot else {
            return Err(format!(
                "physics provider snapshot is unavailable while binding backend '{}'; physics backends must be validated as service plugins with '{}' and '{}'",
                info.backend_id,
                PHYSICS_BACKEND_CAPABILITY_ID,
                PHYSICS_SERVICE_ID,
            ));
        };

        let provider = snapshot.plugins.iter().find(|plugin| {
            plugin.id == info.backend_id
                && plugin_declares_physics_backend(plugin)
                && plugin_declares_service(plugin, PHYSICS_SERVICE_ID)
        });

        if let Some(plugin) = provider {
            return Ok(PhysicsBackendSelection {
                provider_plugin_id: plugin.id.clone(),
                provider_state: plugin.state.clone(),
                matched_by: "plugin-id+capability:physics.backend+service:physics.api",
            });
        }

        let physics_plugins: Vec<String> = snapshot
            .plugins
            .iter()
            .filter(|plugin| {
                plugin_declares_physics_backend(plugin)
                    || plugin_declares_service(plugin, PHYSICS_SERVICE_ID)
            })
            .map(|plugin| {
                let has_backend_cap = plugin_declares_physics_backend(plugin);
                let has_service = plugin_declares_service(plugin, PHYSICS_SERVICE_ID);
                format!(
                    "{}:{} backend_cap={} service={}",
                    plugin.id, plugin.state, has_backend_cap, has_service
                )
            })
            .collect();

        Err(format!(
            "active physics service backend '{}' is not backed by a loaded plugin declaring both '{}' and '{}'; available physics providers=[{}]",
            info.backend_id,
            PHYSICS_BACKEND_CAPABILITY_ID,
            PHYSICS_SERVICE_ID,
            physics_plugins.join(", "),
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
pub(crate) fn plugin_declares_physics_backend(plugin: &PluginSnapshotEntry) -> bool {
    plugin.capabilities.iter().any(|cap| {
        cap.role == CapabilityRole::Provides
            && cap.id.as_str() == PHYSICS_BACKEND_CAPABILITY_ID
    })
}
