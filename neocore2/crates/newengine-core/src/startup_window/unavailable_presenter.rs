#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use super::report::StartupWindowReport;

pub(crate) fn present(config_path: &Path) -> StartupWindowReport {
    StartupWindowReport::unavailable_with_warnings(
        Some(config_path.to_path_buf()),
        "PreStart egui presenter is not compiled; continuing with config.json",
        vec!["enable the `startup-window-egui` feature to show the native Rust PreStart window".to_owned()],
    )
}
