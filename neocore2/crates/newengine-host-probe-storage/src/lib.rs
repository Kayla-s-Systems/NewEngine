#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_host_capabilities_api::StorageCapabilities;
use sysinfo::Disks;

/// Discover only mounted storage devices.
pub fn discover() -> Vec<StorageCapabilities> {
    Disks::new_with_refreshed_list()
        .list()
        .iter()
        .map(|disk| StorageCapabilities {
            name: disk.name().to_string_lossy().into_owned(),
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
            file_system: Some(disk.file_system().to_string_lossy().into_owned()),
            total_bytes: Some(disk.total_space()),
            available_bytes: Some(disk.available_space()),
            removable: Some(disk.is_removable()),
        })
        .collect()
}
