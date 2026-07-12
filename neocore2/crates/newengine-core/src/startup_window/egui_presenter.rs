#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use eframe::egui;
use newengine_loading_api::bootstrap_ui::north_star_bootstrap_ui_style;

use crate::startup::StartupConfig;

use super::report::{StartupWindowReport, StartupWindowSelection};

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

pub(super) const APP_TITLE: &str = north_star_bootstrap_ui_style().brand.prestart_title;
pub(super) const APP_SUBTITLE: &str = north_star_bootstrap_ui_style().brand.prestart_subtitle;
pub(super) const APP_TAGLINE: &str = north_star_bootstrap_ui_style().brand.tagline;
pub(super) const WINDOW_WIDTH: f32 = 1180.0;
pub(super) const WINDOW_HEIGHT: f32 = 790.0;
pub(super) const MIN_WINDOW_WIDTH: f32 = 980.0;
pub(super) const MIN_WINDOW_HEIGHT: f32 = 680.0;
pub(super) const SIDEBAR_WIDTH: f32 = 246.0;

pub(crate) fn present(config_path: &Path, startup: &StartupConfig) -> StartupWindowReport {
    let report_path = config_path.to_path_buf();
    let outcome = Arc::new(Mutex::new(PresenterOutcome::Pending));
    let outcome_for_app = Arc::clone(&outcome);
    let startup = startup.clone();
    let config_path = config_path.to_path_buf();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("{APP_TITLE} — PreStart"))
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT])
            .with_resizable(true),
        ..Default::default()
    };

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
            "newengine-core Egui PreStart presenter failed: {err}"
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
                "newengine-core PreStart graphics settings confirmed and persisted",
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
