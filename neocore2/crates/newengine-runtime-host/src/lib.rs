#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "full-runtime")]
mod host_early_log;
#[cfg(feature = "full-runtime")]
pub mod app_launcher;
#[cfg(feature = "full-runtime")]
pub mod engine_factory;
#[cfg(feature = "full-runtime")]
pub mod headless_cli;
pub mod path_display;
#[cfg(feature = "full-runtime")]
pub mod path_resolver;
#[cfg(feature = "full-runtime")]
mod threading_gateway;
#[cfg(feature = "full-runtime")]
pub mod runtime_config;
#[cfg(feature = "full-runtime")]
pub mod preinit;
#[cfg(feature = "full-runtime")]
pub use newengine_host_capabilities_api::{HostCapabilities, HostPreInitSnapshot, RuntimeCapabilityPolicy};
#[cfg(feature = "full-runtime")]
pub use threading_gateway::register_threading_gateway_service_best_effort;
#[cfg(feature = "full-runtime")]
pub use headless_cli::{HeadlessCliRuntime, HeadlessRuntimeFrontend};
