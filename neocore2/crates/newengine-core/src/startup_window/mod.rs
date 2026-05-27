#![forbid(unsafe_op_in_unsafe_fn)]

//! PreStart configuration diagnostics.
//!
//! North Star UI rendering is owned by `engine.ui`; core must not open a
//! separate native configuration window before the UI provider route exists.
//! This module resolves the config path and records an unavailable report so
//! startup can continue with explicit diagnostics instead of drawing a bypass UI.

mod args;
mod config_path;
mod unavailable_presenter;
mod report;

pub use report::{StartupWindowDecision, StartupWindowReport};

use crate::startup::ConfigPaths;

/// Resolves PreStart configuration state before `StartupLoader` consumes
/// `config.json`, unless the process args explicitly disable it. UI drawing is
/// intentionally not performed here; `engine.ui` is the only UI render path.
pub fn present_before_startup_if_needed(paths: &ConfigPaths) -> StartupWindowReport {
    if let Some(disabled_by) = args::disabled_by_process_args_or_env() {
        return StartupWindowReport::skipped(disabled_by);
    }

    let config_path = match config_path::resolve_for_edit(paths.startup_path()) {
        Ok(path) => path,
        Err(err) => {
            return StartupWindowReport::unavailable(format!(
                "startup window config path resolution failed: {err}"
            ));
        }
    };

    unavailable_presenter::present(&config_path)
}
