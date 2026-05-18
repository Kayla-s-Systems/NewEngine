#![forbid(unsafe_op_in_unsafe_fn)]

mod bootstrap_overlay;
mod callbacks;
mod config;
mod constants;
mod discovery;
mod early_log;
mod handles;
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
pub use types::ResolvedPlatformRuntimeConfig;
