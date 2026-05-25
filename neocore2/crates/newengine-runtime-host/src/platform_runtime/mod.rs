#![forbid(unsafe_op_in_unsafe_fn)]

mod bootstrap_overlay;
mod callbacks;
mod config;
mod constants;
mod discovery;
mod early_log;
mod handles;
mod jobs_gateway;
mod loading_gateway;
mod runtime_host;
mod snapshot_service;
mod shutdown_watchdog;
mod types;
mod ui_gateway_frame;
mod ui_provider_selection;

pub use config::{
    platform_config_from_startup_defaults,
    resolve_platform_runtime_config,
};
pub use discovery::detect_platform_runtime_path;
pub use runtime_host::HostPlatformRuntime;
pub(crate) use jobs_gateway::register_jobs_gateway_service_best_effort;
pub(crate) use loading_gateway::register_loading_gateway_service_best_effort;
pub use types::ResolvedPlatformRuntimeConfig;
