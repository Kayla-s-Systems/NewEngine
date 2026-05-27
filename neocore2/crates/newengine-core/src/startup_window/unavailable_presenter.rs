#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use super::report::StartupWindowReport;

pub(crate) fn present(config_path: &Path) -> StartupWindowReport {
    StartupWindowReport::unavailable_with_warnings(
        Some(config_path.to_path_buf()),
        "PreStart UI renderer is not available through engine.ui; continuing with config.json",
        vec!["no native or special PreStart renderer is allowed; publish startup UI through engine.ui when a provider route exists".to_owned()],
    )
}
