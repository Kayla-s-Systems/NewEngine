#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_host_capabilities_api::MemoryCapabilities;
use sysinfo::System;

/// Discover only total host memory.
pub fn discover() -> MemoryCapabilities {
    let mut sys = System::new_all();
    sys.refresh_memory();
    MemoryCapabilities {
        total_mb: (sys.total_memory() > 0).then(|| sys.total_memory() / (1024 * 1024)),
    }
}
