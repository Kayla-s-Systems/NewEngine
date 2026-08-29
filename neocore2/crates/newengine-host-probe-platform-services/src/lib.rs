#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_host_capabilities_api::HostPlatformServices;

/// Discover only host primitive availability. No hardware inventory or policy.
pub fn discover() -> HostPlatformServices {
    HostPlatformServices {
        native_threads: std::thread::available_parallelism().is_ok(),
        filesystem: true,
        process_environment: true,
        dynamic_library_loading: matches!(std::env::consts::OS, "windows" | "linux" | "macos"),
    }
}
