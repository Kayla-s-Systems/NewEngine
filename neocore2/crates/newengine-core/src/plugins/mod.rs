#![forbid(unsafe_op_in_unsafe_fn)]

mod control;
mod describe;
mod forward_logger;
pub(crate) mod host_api;
pub mod host_context;
mod manager;
mod paths;
pub(crate) mod plugin_config_service;

/// A lightweight snapshot of loaded plugins suitable for UI/telemetry.
///
/// This is produced by the host (engine) and stored in `Resources` each frame.
#[derive(Clone, Debug, Default)]
pub struct PluginsSnapshot {
    pub plugins: Vec<PluginSnapshotEntry>,
}

pub use control::{PluginControlCommand, PluginControlQueue, PluginControlResult};
pub use forward_logger::{install_forward_logger_once, LOGGING_SINK_SERVICE_ID};
pub use host_api::default_host_api;
pub use host_context::{has_service, init_host_context, list_services};
pub use manager::{PluginManager, PluginSnapshotEntry};
pub use plugin_config_service::{init_plugin_config_service, CONFIG_SERVICE_ID};
