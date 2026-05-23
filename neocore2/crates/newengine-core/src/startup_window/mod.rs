#![forbid(unsafe_op_in_unsafe_fn)]

//! Core-owned PreStart configuration workbench.
//!
//! The startup window is a host/startup concern: it opens before plugin loading,
//! renderer creation and platform window creation, edits canonical `config.json`,
//! then lets `StartupLoader` read the saved configuration. It is intentionally
//! not a `tools/` script and does not depend on the current working directory.

mod args;
mod config_path;
#[cfg(feature = "startup-window-egui")]
mod prestart_asset_resolver;
#[cfg(feature = "startup-window-egui")]
mod icons;
#[cfg(feature = "startup-window-egui")]
mod svg_assets;
#[cfg(feature = "startup-window-egui")]
mod egui_presenter;
#[cfg(not(feature = "startup-window-egui"))]
mod unavailable_presenter;
mod report;

pub use report::{StartupWindowDecision, StartupWindowReport};

use crate::startup::ConfigPaths;

/// Presents the PreStart configuration workbench before `StartupLoader`
/// consumes `config.json`, unless the process args explicitly disable it.
///
/// Disable flags are intentionally process-level switches because the default
/// behavior for desktop launches is to show the configuration window.
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

    #[cfg(feature = "startup-window-egui")]
    {
        egui_presenter::present(&config_path)
    }

    #[cfg(not(feature = "startup-window-egui"))]
    {
        unavailable_presenter::present(&config_path)
    }
}
