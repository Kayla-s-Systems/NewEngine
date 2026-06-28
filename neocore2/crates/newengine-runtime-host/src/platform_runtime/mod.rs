#![forbid(unsafe_op_in_unsafe_fn)]

mod bootstrap_overlay;
mod bootstrap_subsystems;
mod callbacks;
mod config;
mod constants;
mod discovery;
mod early_log;
mod fatal_overlay;
mod handles;
mod jobs_gateway;
mod runtime_host;
mod screen_profile;
mod shutdown_watchdog;
mod snapshot_service;
mod types;
mod ui_gateway_frame;
mod ui_provider_selection;

pub use config::{platform_config_from_startup_defaults, resolve_platform_runtime_config};
pub use discovery::detect_platform_runtime_path;
pub(crate) use jobs_gateway::register_jobs_gateway_service_best_effort;
pub use runtime_host::HostPlatformRuntime;
pub use types::ResolvedPlatformRuntimeConfig;
