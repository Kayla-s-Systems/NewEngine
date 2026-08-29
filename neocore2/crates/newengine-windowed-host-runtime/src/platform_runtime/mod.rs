#![forbid(unsafe_op_in_unsafe_fn)]

mod boot_presenter;
mod bootstrap_overlay;
mod bootstrap_subsystems;
mod callbacks;
mod config;
mod console_overlay;
mod constants;
mod discovery;
pub(crate) mod early_log;
mod fatal_overlay;
mod handles;
mod runtime_host;
mod screen_profile;
mod shutdown_watchdog;
mod snapshot_service;
mod types;
mod ui_gateway_frame;
mod ui_layer_cache;
mod ui_provider_selection;

pub use config::{platform_config_from_startup_defaults, resolve_platform_runtime_config};
pub use discovery::detect_platform_runtime_path;
pub use runtime_host::HostPlatformRuntime;
pub use types::ResolvedPlatformRuntimeConfig;
