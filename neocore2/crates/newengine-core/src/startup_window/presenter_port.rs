#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::sync::OnceLock;

use crate::startup::StartupConfig;

use super::StartupWindowReport;

/// Host-installed concrete presenter entry point. Core owns the contract, not the toolkit.
pub type StartupWindowPresenterFn = fn(&Path, &StartupConfig) -> StartupWindowReport;

static STARTUP_WINDOW_PRESENTER: OnceLock<StartupWindowPresenterFn> = OnceLock::new();

/// Installs the process PreStart presenter. Registration is first-wins so product
/// composition remains deterministic; repeated install attempts are harmless.
pub fn install_startup_window_presenter(presenter: StartupWindowPresenterFn) -> bool {
    STARTUP_WINDOW_PRESENTER.set(presenter).is_ok()
}

#[inline]
pub fn startup_window_presenter_registered() -> bool {
    STARTUP_WINDOW_PRESENTER.get().is_some()
}

pub(crate) fn present(
    config_path: &Path,
    startup: &StartupConfig,
) -> Option<StartupWindowReport> {
    STARTUP_WINDOW_PRESENTER
        .get()
        .copied()
        .map(|presenter| presenter(config_path, startup))
}
