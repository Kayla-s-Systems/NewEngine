#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use eframe::egui;
use newengine_loading_api::bootstrap_ui::{north_star_bootstrap_ui_style, BootstrapUiRgb};
use serde_json::{Map, Number, Value};

use super::icons::{icon, IconKind};
use super::report::StartupWindowReport;
use super::svg_assets::SvgIconRegistry;

const APP_TITLE: &str = north_star_bootstrap_ui_style().brand.prestart_title;
const APP_SUBTITLE: &str = north_star_bootstrap_ui_style().brand.prestart_subtitle;
const SCHEMA_LABEL: &str = "newengine.startup_window.v1";
const ACCENT_BLUE: egui::Color32 = egui::Color32::from_rgb(78, 153, 255);
const ACCENT_BLUE_BRIGHT: egui::Color32 = egui::Color32::from_rgb(106, 181, 255);
const ACCENT_GREEN: egui::Color32 = egui::Color32::from_rgb(121, 232, 123);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(232, 238, 250);
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(146, 158, 184);
const WINDOW_WIDTH: f32 = 1320.0;
const WINDOW_HEIGHT: f32 = 860.0;
const MIN_WINDOW_WIDTH: f32 = 980.0;
const MIN_WINDOW_HEIGHT: f32 = 680.0;
const CENTER_ATTEMPT_LIMIT: u8 = 8;

mod app_lifecycle;
mod chrome;
mod config_store;
mod panels;
mod persistence;
mod plugin_widgets;
mod shell;
mod widgets;

use self::config_store::*;
use self::plugin_widgets::*;
use self::shell::*;
use self::widgets::*;

#[inline]
fn color32(rgb: BootstrapUiRgb) -> egui::Color32 {
    egui::Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowOutcome {
    None,
    LaunchRequested,
    Cancelled,
}

pub(crate) fn present(config_path: &Path) -> StartupWindowReport {
    let title = format!("{APP_TITLE} PreStart — {}", env!("CARGO_PKG_VERSION"));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT])
            .with_resizable(true),
        ..Default::default()
    };

    let config_path = config_path.to_path_buf();
    let report_path = config_path.clone();
    let outcome = Arc::new(Mutex::new(WindowOutcome::None));
    let outcome_for_app = outcome.clone();
    let result = eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(PreStartApp::new(cc, config_path.clone(), outcome_for_app.clone())))),
    );

    match result {
        Ok(()) => match outcome.lock().map(|g| *g).unwrap_or(WindowOutcome::None) {
            WindowOutcome::LaunchRequested => StartupWindowReport::presented(
                report_path,
                "core-owned Rust/egui PreStart window launched by explicit user action",
                Vec::new(),
            ),
            WindowOutcome::Cancelled | WindowOutcome::None => StartupWindowReport::cancelled(
                report_path,
                "PreStart window was closed or cancelled; launch is allowed only from the Launch Engine button",
            ),
        },
        Err(err) => StartupWindowReport::unavailable_with_warnings(
            Some(report_path),
            format!("egui PreStart window failed: {err}; continuing with config.json"),
            vec![format!("egui presenter error: {err}")],
        ),
    }
}

#[derive(Clone, Debug)]
struct SelectOption {
    value: String,
    label: String,
}

#[derive(Clone, Debug)]
struct SchemaField {
    key: String,
    path: String,
    label: String,
    kind: String,
    options: Vec<SelectOption>,
    default_label: Option<String>,
}

#[derive(Clone, Debug)]
struct PluginTab {
    plugin_id: String,
    title: String,
    category: String,
    source: String,
    enabled: bool,
    fields: Vec<SchemaField>,
}

#[derive(Default)]
struct FieldStore {
    strings: HashMap<String, String>,
    bools: HashMap<String, bool>,
    selects: HashMap<String, String>,
}

impl FieldStore {
    fn string(&mut self, key: &str, default: impl Into<String>) -> &mut String {
        self.strings.entry(key.to_owned()).or_insert_with(|| default.into())
    }

    fn bool(&mut self, key: &str, default: bool) -> &mut bool {
        self.bools.entry(key.to_owned()).or_insert(default)
    }

    fn select(&mut self, key: &str, default: impl Into<String>) -> &mut String {
        self.selects.entry(key.to_owned()).or_insert_with(|| default.into())
    }

    fn string_value(&self, key: &str, default: &str) -> String {
        self.strings.get(key).cloned().unwrap_or_else(|| default.to_owned())
    }

    fn bool_value(&self, key: &str, default: bool) -> bool {
        self.bools.get(key).copied().unwrap_or(default)
    }

    fn select_value(&self, key: &str, default: &str) -> String {
        self.selects.get(key).cloned().unwrap_or_else(|| default.to_owned())
    }
}

struct PreStartApp {
    config_path: PathBuf,
    config: Value,
    parse_warning: Option<String>,
    selected_tab: usize,
    selected_plugin: Option<String>,
    fields: FieldStore,
    plugin_tabs: Vec<PluginTab>,
    status: String,
    style_ready: bool,
    svg_icons: SvgIconRegistry,
    center_attempts: u8,
    outcome: Arc<Mutex<WindowOutcome>>,
}
