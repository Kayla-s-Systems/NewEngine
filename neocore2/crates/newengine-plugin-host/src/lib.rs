#![forbid(unsafe_op_in_unsafe_fn)]
pub mod content_manifest;
mod control;
mod diagnostics;
pub mod host_api;
pub mod host_context;
mod manager;
mod paths;
mod plugin_config_service;
mod root_observers;
mod service_gateway;
pub mod ulog_event;

/// A lightweight snapshot of loaded plugins suitable for UI/telemetry.
///
/// This is produced by the host (engine) only when plugin state changes.
/// Cloning the snapshot is O(1): plugin entries are retained behind `Arc`.
#[derive(Clone, Debug, Default)]
pub struct PluginsSnapshot {
    pub revision: u64,
    pub plugins: std::sync::Arc<[PluginSnapshotEntry]>,
}

impl PluginsSnapshot {
    /// True only when a currently running plugin explicitly provides `capability_id`.
    /// Tool availability must be derived from composition state, never launch-profile names.
    pub fn has_running_capability(&self, capability_id: &str) -> bool {
        let capability_id = capability_id.trim();
        !capability_id.is_empty()
            && self.plugins.iter().any(|plugin| {
                plugin.state == "running"
                    && plugin.capabilities.iter().any(|capability| {
                        capability.role == newengine_plugin_api::CapabilityRole::Provides
                            && capability.id.as_str() == capability_id
                    })
            })
    }
}

pub use content_manifest::{
    load_plugin_content_catalog_default, load_plugin_content_catalog_from_dir, PluginContentBlob,
    PluginContentCatalog, PluginContentLoadReport,
};
pub use control::{PluginControlCommand, PluginControlQueue, PluginControlResult};
pub use host_api::{call_service_v1, default_host_api, host_register_service_impl};
pub use host_context::{
    activate_host_context, active_engine_gateway_route, clear_engine_gateway_selection_policies,
    create_host_context, create_host_context_with_environment_snapshot, current_host_context,
    declare_engine_capability_requirement, declare_engine_capability_slot,
    declare_engine_composition, describe_service, engine_composition_allows_system_tags,
    engine_composition_explanation, engine_composition_has_forbidden_system_tags,
    engine_composition_snapshot_v1, engine_composition_snapshot_v1_json,
    engine_gateway_has_capability, explain_engine_gateway_composition, has_service,
    init_host_context, install_engine_gateway_selection_policy, list_engine_capability_slots,
    list_engine_gateway_routes, list_external_runtime_descriptors, list_external_runtime_plugins,
    list_runtime_contracts, list_services, register_engine_gateway_provider_route,
    register_external_runtime_plugin, register_null_engine_gateway_provider_route,
    register_null_engine_gateway_provider_route_with_abi,
    register_null_engine_gateway_provider_route_with_abi_and_tags,
    register_null_engine_gateway_provider_route_with_tags, resolve_service_for_backend_capability,
    resolve_service_for_engine_gateway, runtime_contract, runtime_contract_by_advertised_id,
    services_generation, validate_required_engine_capability_slots, with_host_context,
    with_host_module_callback, EngineCapabilitySlotSnapshot, EngineGatewayRouteSnapshot,
    EngineGatewaySelectionPolicy, HostContextHandle, ProviderRegistrationTransaction,
    RuntimeContractAuthority, RuntimeContractEntry, RuntimeContractSpec,
};
/// Reads declarative plugin discovery metadata and fingerprints the DLL without mapping it.
pub fn read_verified_plugin_discovery_manifest(
    path: &std::path::Path,
) -> Result<newengine_plugin_api::PluginDiscoveryManifestV2, String> {
    manager::read_verified_manifest(path).map(|snapshot| snapshot.manifest)
}

pub use manager::{
    resolve_plugin_discovery_dir, scan_plugin_discovery_graph, IncrementalLoadOutcome,
    PluginDiscoveryGraph, PluginIconSnapshot, PluginLoadError, PluginLoadOrigin, PluginManager,
    PluginRuntimeUnitInventoryEntry, PluginSnapshotEntry,
};
pub use plugin_config_service::{
    get_plugin_overrides_with_env, init_plugin_config_service, CONFIG_SERVICE_ID,
};
pub use root_observers::{
    editor_extensions_snapshot_v1, register_plugin_root_observer, LoadedPluginRootSnapshot,
    PluginEditorExtensionsExport, PluginRootObserver,
};
pub use service_gateway::{descriptor_gateway_capabilities, EngineGatewayCapability};

/// Publishes a plugin-host event into all subscribed plugin event sinks.
///
/// This is the *host-side* entrypoint. The ABI-facing entrypoint is `HostApiV1.emit_event_v1`.
#[inline]
pub fn emit_plugin_event(topic: &str, payload: &[u8]) -> Result<(), String> {
    host_context::publish_event(topic, payload)
}

/// Convenience wrapper: publishes a JSON value as a plugin-host event.
#[inline]
pub fn emit_plugin_json(topic: &str, value: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    emit_plugin_event(topic, &bytes)
}
