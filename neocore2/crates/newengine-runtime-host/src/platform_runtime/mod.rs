#![forbid(unsafe_op_in_unsafe_fn)]

mod callbacks;
mod config;
mod constants;
mod discovery;
mod handles;
mod runtime_host;
mod snapshot_service;
mod types;

pub use config::{
    legacy_platform_config_from_startup,
    resolve_platform_runtime_config,
};
pub use discovery::detect_platform_runtime_path;
pub use runtime_host::HostPlatformRuntime;
pub use types::ResolvedPlatformRuntimeConfig;
