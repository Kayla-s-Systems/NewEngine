#![forbid(unsafe_op_in_unsafe_fn)]

//! Core-owned PreStart launch configuration.
//!
//! The feature is compiled into `newengine-core` through the optional
//! `startup-window-egui` feature and is requested at runtime through the
//! runtime-host `PreStartConfigWindow` boot option. No separate settings crate
//! is involved.
//!
//! Core owns the typed settings model, validation, persistence contract,
//! process variables and the launch/cancel decision. Platform and render
//! runtimes consume the confirmed core snapshot after this phase.

mod args;
mod config_path;
#[cfg(feature = "startup-window-egui")]
mod egui_presenter;
mod loading_handoff;
mod report;
mod settings;

pub use report::{
    StartupLoadingAssignmentReport, StartupWindowDecision, StartupWindowReport,
    StartupWindowSelection,
};
pub use settings::{
    startup_launch_settings, GraphicsPreset, ShadowQuality, StartupDisplaySettings,
    StartupGraphicsSettings, StartupHdrMode, StartupLaunchSettings, StartupWindowMode,
    TextureQuality, STARTUP_SETTINGS_SCHEMA_VERSION,
};

use crate::startup::{ConfigPaths, StartupConfig};

/// Presents the core-owned PreStart settings window before Engine creation when
/// the launch contract requests it. Closing or cancelling returns
/// `StartupWindowDecision::Cancelled` and never persists editor state.
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

    let loading_handoff = loading_handoff::present(&config_path, startup);

    #[cfg(feature = "startup-window-egui")]
    let mut report = egui_presenter::present(&config_path, startup);

    #[cfg(not(feature = "startup-window-egui"))]
    let mut report = StartupWindowReport::unavailable(
        "PreStart configuration was requested, but newengine-core was built without the 'startup-window-egui' feature",
    );

    report.attach_loading_handoff(loading_handoff);
    report
}

pub(crate) use settings::set_startup_launch_settings;
