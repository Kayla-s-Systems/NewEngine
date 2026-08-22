#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use eframe::egui;
use newengine_loading_api::bootstrap_ui::north_star_bootstrap_ui_style;

use newengine_core::startup::StartupConfig;

use newengine_core::startup_window::{StartupWindowReport, StartupWindowSelection};

/// Installs the optional Egui presenter into the core-owned PreStart presenter port.
/// Registration is first-wins; repeated bootstrap calls are harmless.
pub fn install() -> bool {
    newengine_core::startup_window::install_startup_window_presenter(present)
}

mod app;
mod chrome;
mod model;
mod panels;
mod persistence;
mod style;
mod widgets;

use app::PreStartGraphicsApp;
use model::PresenterOutcome;
use style::configure_style;

pub(crate) const APP_TITLE: &str = north_star_bootstrap_ui_style().brand.prestart_title;
pub(crate) const APP_SUBTITLE: &str = north_star_bootstrap_ui_style().brand.prestart_subtitle;
pub(crate) const APP_TAGLINE: &str = north_star_bootstrap_ui_style().brand.tagline;
pub(crate) const WINDOW_WIDTH: f32 = 1180.0;
pub(crate) const WINDOW_HEIGHT: f32 = 720.0;
pub(crate) const MIN_WINDOW_WIDTH: f32 = 960.0;
pub(crate) const MIN_WINDOW_HEIGHT: f32 = 620.0;
pub(crate) const SIDEBAR_WIDTH: f32 = 212.0;

fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("{APP_TITLE} - Launch Configuration"))
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT])
            .with_clamp_size_to_monitor_size(true)
            .with_resizable(true),
        // PreStart is a transient engine-owned gate. Its position is not user state:
        // each invocation should open predictably in the center of the active desktop.
        centered: true,
        persist_window: false,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prestart_window_is_centered_and_not_position_persistent() {
        let options = native_options();
        assert!(options.centered);
        assert!(!options.persist_window);
        assert_eq!(
            options.viewport.inner_size,
            Some(egui::vec2(WINDOW_WIDTH, WINDOW_HEIGHT))
        );
        assert_eq!(
            options.viewport.min_inner_size,
            Some(egui::vec2(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT))
        );
        assert_eq!(options.viewport.clamp_size_to_monitor_size, Some(true));
    }
}

fn present(config_path: &Path, startup: &StartupConfig) -> StartupWindowReport {
    let report_path = config_path.to_path_buf();
    let outcome = Arc::new(Mutex::new(PresenterOutcome::Pending));
    let outcome_for_app = Arc::clone(&outcome);
    let startup = startup.clone();
    let config_path = config_path.to_path_buf();

    let options = native_options();

    let result = eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |cc| {
            configure_style(&cc.egui_ctx);
            Ok(Box::new(PreStartGraphicsApp::new(
                config_path,
                startup,
                outcome_for_app,
            )))
        }),
    );

    if let Err(err) = result {
        return StartupWindowReport::unavailable(format!(
            "newengine-startup-window-egui presenter failed: {err}"
        ));
    }

    match outcome
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or(PresenterOutcome::Cancelled)
    {
        PresenterOutcome::Confirmed(selection) => {
            StartupWindowReport::presented_with_selection(
                report_path,
                "newengine-startup-window-egui settings confirmed and persisted",
                Vec::new(),
                selection,
            )
        }
        PresenterOutcome::Pending | PresenterOutcome::Cancelled => StartupWindowReport::cancelled(
            report_path,
            "PreStart settings window was closed or cancelled; last confirmed settings were not changed",
        ),
    }
}
