#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use crate::loading::{BootViewport, EngineLoadingKernel, LoadingPhase};
use crate::startup::StartupConfig;

use super::report::StartupWindowReport;

pub(crate) fn present(config_path: &Path, startup: &StartupConfig) -> StartupWindowReport {
    let mut loading_kernel = EngineLoadingKernel::with_startup_config(startup);
    let assignment = loading_kernel.resolve_assignment(LoadingPhase::PreStart);
    let boot_frame = loading_kernel.boot_frame(BootViewport::default());

    StartupWindowReport::presented_with_loading_assignment(
        config_path.to_path_buf(),
        "native window configured loading_assignment_owner='engine.loading' prestart_ui_dependency='none' role='native window/display config only'",
        Vec::new(),
        assignment,
        boot_frame,
    )
}
