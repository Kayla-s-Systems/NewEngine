#![recursion_limit = "256"]
#![forbid(unsafe_op_in_unsafe_fn)]

mod frontend;
pub mod platform_input;
pub mod platform_runtime;

pub use frontend::{WindowedHostFrontend, WindowedRuntimeHostProfile};
pub use platform_runtime::{
    detect_platform_runtime_path, resolve_platform_runtime_config, HostPlatformRuntime,
    ResolvedPlatformRuntimeConfig,
};
