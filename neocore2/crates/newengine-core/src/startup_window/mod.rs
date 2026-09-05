#![forbid(unsafe_op_in_unsafe_fn)]

//! Core-owned PreStart launch configuration.
//!
//! `newengine-core` owns the typed settings/report contract and a presenter port.
//! Concrete presentation toolkits are upper-layer host providers; an empty/Void
//! Engine host links no Egui, windowing presenter, or product UI implementation.
//!
//! Core owns the typed settings model, validation, persistence contract,
//! process variables and the launch/cancel decision. Platform and render
//! runtimes consume the confirmed core snapshot after this phase.

mod args;
mod config_path;
mod loading_handoff;
mod presenter_port;
mod report;
mod settings;

pub use presenter_port::{
    install_startup_window_presenter, startup_window_presenter_registered, StartupWindowPresenterFn,
};
pub use report::{
    StartupLoadingAssignmentReport, StartupWindowDecision, StartupWindowReport,
    StartupWindowSelection,
};
pub use settings::{
    startup_launch_settings, GraphicsPreset, LodQuality, ShadowFilterMode, ShadowQuality,
    StartupDisplaySettings, StartupGraphicsSettings, StartupHdrMode, StartupLaunchSettings,
    StartupWindowMode, TextureQuality, ENV_LOD_DISTANCE_SCALE, ENV_SHADOWS_ENABLED,
    ENV_SHADOW_CASCADE_COUNT, ENV_SHADOW_MAP_RESOLUTION, ENV_VIEW_DISTANCE_METERS,
    STARTUP_SETTINGS_SCHEMA_VERSION,
};

use crate::startup::{ConfigPaths, StartupConfig};

/// Presents the core-owned PreStart settings contract before Engine creation.
/// A concrete presenter is supplied by the upper host composition. Closing or
/// cancelling returns `StartupWindowDecision::Cancelled` and never persists editor state.
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
    let mut report = presenter_port::present(&config_path, startup).unwrap_or_else(|| {
        StartupWindowReport::unavailable(
            "PreStart configuration was requested, but no startup-window presenter provider is registered",
        )
    });

    report.attach_loading_handoff(loading_handoff);
    report
}

pub(crate) use settings::set_startup_launch_settings;
