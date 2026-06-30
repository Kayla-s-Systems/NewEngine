#![forbid(unsafe_op_in_unsafe_fn)]

//! PreStart loading lifecycle diagnostics.
//!
//! `startup_window` owns native window/display configuration only. Loading
//! assignment is resolved by the core-owned `engine.loading` domain and then
//! handed off to platform/runtime presenters. PreStart must not depend on a
//! runtime UI provider route.

mod args;
mod config_path;
mod loading_handoff;
mod report;

pub use report::{StartupLoadingAssignmentReport, StartupWindowDecision, StartupWindowReport};

use crate::startup::{ConfigPaths, StartupConfig};

/// Resolves PreStart configuration state before `StartupLoader` consumes
/// `config.json`, unless the process args explicitly disable it. UI drawing is
/// intentionally not performed here; this records the engine.loading handoff
/// contract so diagnostics show ownership and dependency boundaries.
pub fn present_before_startup_if_needed(
    paths: &ConfigPaths,
    startup: &StartupConfig,
) -> StartupWindowReport {
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

    loading_handoff::present(&config_path, startup)
}
