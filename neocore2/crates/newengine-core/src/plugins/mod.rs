#![forbid(unsafe_op_in_unsafe_fn)]

mod describe;
pub(crate) mod host_api;
pub mod host_context;
mod forward_logger;
mod control;
mod manager;
mod paths;
pub(crate) mod startup_config_service;

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
pub use host_context::init_host_context;
pub use manager::PluginManager;
pub use manager::PluginSnapshotEntry;
