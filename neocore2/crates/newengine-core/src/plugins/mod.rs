#![forbid(unsafe_op_in_unsafe_fn)]

mod describe;
mod host_api;
pub(crate) mod host_context;
mod importer;
mod manager;
mod paths;
mod service_registry;

pub use host_api::{default_host_api, importers_host_api};
pub use host_context::init_host_context;
pub use manager::{PluginLoadError, PluginManager};

pub(crate) use service_registry::{ServiceEntrySnapshot, ServiceId, ServiceRegistry};
