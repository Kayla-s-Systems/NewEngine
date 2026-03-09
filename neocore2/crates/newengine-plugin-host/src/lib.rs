#![forbid(unsafe_op_in_unsafe_fn)]

mod control;
mod describe;
pub mod host_api;
pub mod host_context;
mod log_fmt;
mod manager;
mod path_fmt;
mod paths;
mod plugin_config_service;

/// A lightweight snapshot of loaded plugins suitable for UI/telemetry.
///
/// This is produced by the host (engine) and stored in `Resources` each frame.
#[derive(Clone, Debug, Default)]
pub struct PluginsSnapshot {
    pub plugins: Vec<PluginSnapshotEntry>,
}

pub use control::{PluginControlCommand, PluginControlQueue, PluginControlResult};
pub use host_api::{call_service_v1, default_host_api, host_register_service_impl};
pub use host_context::{
    describe_service, has_service, init_host_context, list_external_runtime_descriptors,
    list_external_runtime_plugins, list_services, register_external_runtime_plugin,
    services_generation,
};
pub use manager::{PluginIconSnapshot, PluginLoadError, PluginManager, PluginSnapshotEntry};
pub use plugin_config_service::{
    get_plugin_overrides_with_env, init_plugin_config_service, CONFIG_SERVICE_ID,
};

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
