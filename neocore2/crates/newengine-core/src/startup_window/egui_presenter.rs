#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use eframe::egui;
use serde_json::{Map, Number, Value};

use super::icons::{icon, IconKind};
use super::report::StartupWindowReport;
use super::svg_assets::SvgIconRegistry;

const APP_TITLE: &str = "PreStart Engine";
const APP_SUBTITLE: &str = "Launch configuration workbench";
const SCHEMA_LABEL: &str = "newengine.startup_window.v1";
const ACCENT_BLUE: egui::Color32 = egui::Color32::from_rgb(78, 153, 255);
const ACCENT_BLUE_BRIGHT: egui::Color32 = egui::Color32::from_rgb(106, 181, 255);
const ACCENT_GREEN: egui::Color32 = egui::Color32::from_rgb(121, 232, 123);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(232, 238, 250);
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(146, 158, 184);
const WINDOW_WIDTH: f32 = 1440.0;
const WINDOW_HEIGHT: f32 = 900.0;
const MIN_WINDOW_WIDTH: f32 = 1220.0;
const MIN_WINDOW_HEIGHT: f32 = 760.0;
const CENTER_ATTEMPT_LIMIT: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowOutcome {
    None,
    LaunchRequested,
    Cancelled,
}

pub(crate) fn present(config_path: &Path) -> StartupWindowReport {
    let title = format!("{APP_TITLE} — NewEngine {}", env!("CARGO_PKG_VERSION"));
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

impl PreStartApp {
    fn new(cc: &eframe::CreationContext<'_>, config_path: PathBuf, outcome: Arc<Mutex<WindowOutcome>>) -> Self {
        let (config, parse_warning) = read_config(&config_path);
        let plugin_tabs = collect_plugin_tabs(&config);
        let selected_plugin = plugin_tabs
            .iter()
            .find(|tab| tab.plugin_id != "newengine.engine" && tab.enabled)
            .map(|tab| tab.plugin_id.clone());
        let svg_icons = SvgIconRegistry::load(&config_path, &config);
        let icon_status = svg_icons.source_label();
        let mut app = Self {
            config_path,
            config,
            parse_warning,
            selected_tab: 0,
            selected_plugin,
            fields: FieldStore::default(),
            plugin_tabs,
            status: format!("Ready to launch. {icon_status}. Closing this window will NOT start the engine."),
            style_ready: false,
            svg_icons,
            center_attempts: 0,
            outcome,
        };
        app.seed_builtin_fields();
        app.seed_plugin_fields();
        app.apply_style(&cc.egui_ctx);
        app
    }

    fn set_outcome(&self, outcome: WindowOutcome) {
        if let Ok(mut guard) = self.outcome.lock() {
            *guard = outcome;
        }
    }

    fn open_project_folder(&mut self) {
        let raw = self.fields.string_value("project.path", ".");
        let path = resolve_project_folder(&self.config_path, &raw);
        match open_folder_in_shell(&path) {
            Ok(()) => {
                self.status = format!("Opened project folder {}", path.display());
            }
            Err(err) => {
                self.status = format!("Open Project Folder failed path='{}' err={err}", path.display());
            }
        }
    }

    fn center_window_on_startup(&mut self, ctx: &egui::Context) {
        if self.center_attempts >= CENTER_ATTEMPT_LIMIT {
            return;
        }

        self.center_attempts = self.center_attempts.saturating_add(1);
        let target_position = ctx.input(|input| {
            let viewport = input.viewport();
            let monitor_size = viewport.monitor_size?;
            let rect = match (viewport.outer_rect, viewport.inner_rect) {
                (Some(rect), _) | (None, Some(rect)) => rect,
                (None, None) => return None,
            };
            let size = rect.size();
            if monitor_size.x <= 0.0 || monitor_size.y <= 0.0 || size.x <= 0.0 || size.y <= 0.0 {
                return None;
            }
            Some(egui::pos2(
                ((monitor_size.x - size.x) * 0.5).max(0.0),
                ((monitor_size.y - size.y) * 0.5).max(0.0),
            ))
        });

        if let Some(position) = target_position {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
            self.center_attempts = CENTER_ATTEMPT_LIMIT;
        } else {
            ctx.request_repaint();
        }
    }

    fn apply_style(&mut self, ctx: &egui::Context) {
        if self.style_ready {
            return;
        }
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = egui::Color32::from_rgb(5, 7, 11);
        style.visuals.panel_fill = egui::Color32::from_rgb(7, 10, 15);
        style.visuals.extreme_bg_color = egui::Color32::from_rgb(3, 5, 9);
        style.visuals.faint_bg_color = egui::Color32::from_rgb(16, 20, 29);
        style.visuals.override_text_color = Some(TEXT_PRIMARY);
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(10, 13, 20);
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_MUTED);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(17, 22, 32);
        style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(14, 18, 27);
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 56, 77));
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(24, 34, 52);
        style.visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(20, 28, 43);
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT_BLUE_BRIGHT);
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(31, 58, 102);
        style.visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(28, 48, 83);
        style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT_BLUE_BRIGHT);
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.1, egui::Color32::WHITE);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(34, 89, 176);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_BLUE_BRIGHT);
        style.visuals.widgets.open.bg_fill = egui::Color32::from_rgb(24, 33, 50);
        style.visuals.widgets.open.weak_bg_fill = egui::Color32::from_rgb(18, 25, 38);
        style.visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, ACCENT_BLUE_BRIGHT);
        style.spacing.item_spacing = egui::vec2(11.0, 9.0);
        style.spacing.button_padding = egui::vec2(14.0, 10.0);
        style.spacing.indent = 16.0;
        ctx.set_style(style);
        self.style_ready = true;
    }

    fn seed_builtin_fields(&mut self) {
        self.fields.string("engine.modules_dir", value_string_segments(&self.config, &["engine", "modules_dir"], "plugins"));
        self.fields.string("engine.cache_files", value_string_segments(&self.config, &["engine", "cache_files"], "cache"));
        self.fields.string("engine.config", value_string_segments(&self.config, &["engine", "config"], "config"));
        self.fields.string("project.name", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "project", "name"], "MyGameProject"));
        let project_path = self.config_path.parent().map(|path| path.display().to_string()).unwrap_or_else(|| ".".to_owned());
        self.fields.string("project.path", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "project", "path"], &project_path));
        self.fields.string("launch.parameters", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "launch_parameters"], "--windowed --devmode --log --console"));
        self.fields.string("launch.startup_scene", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "startup_scene"], "MainMenu"));
        self.fields.bool("launch.remember_last_options", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "remember_last_options"], true));

        let title = first_string_segments(&self.config, &[&["plugins", "newengine", "platform.winit", "title"], &["window", "title"]], "Kayla's Editor");
        let width = first_i64_segments(&self.config, &[&["plugins", "newengine", "platform.winit", "width"], &["window", "width"]], 1600);
        let height = first_i64_segments(&self.config, &[&["plugins", "newengine", "platform.winit", "height"], &["window", "height"]], 900);

        self.fields.string("display.title", title);
        self.fields.string("display.width", width.to_string());
        self.fields.string("display.height", height.to_string());
        self.fields.string("display.monitor", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "display", "monitor"], "primary"));
        self.fields.select("display.window_mode", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "display", "window_mode"], "windowed"));
        self.fields.bool("display.fullscreen", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "display", "fullscreen"], false));
        self.fields.bool("display.borderless_fullscreen", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "display", "borderless_fullscreen"], false));
        self.fields.bool("display.vsync", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "display", "vsync"], true));
        self.fields.string("display.refresh_rate", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "display", "refresh_rate"], "auto"));
        self.fields.string("display.render_scale", value_f64_segments(&self.config, &["plugins", "newengine", "startup_window", "display", "render_scale"], 1.0).to_string());
        self.fields.select("display.hdr", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "display", "hdr"], "auto"));

        self.fields.select("graphics.renderer_backend", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "renderer_backend"], "auto"));
        self.fields.string("graphics.gpu_device", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "gpu_device"], "auto"));
        self.fields.select("graphics.graphics_profile", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "graphics_profile"], "auto"));
        self.fields.select("graphics.shader_cache_mode", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "shader_cache_mode"], "auto"));
        self.fields.bool("graphics.debug.enabled", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "enabled"], false));
        self.fields.bool("graphics.debug.renderdoc_capture", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "renderdoc_capture"], false));
        self.fields.bool("graphics.debug.phase_viewer", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "phase_viewer"], false));
        self.fields.bool("graphics.debug.target_viewer", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "target_viewer"], false));
        self.fields.bool("graphics.debug.shadow_cascade_viewer", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "shadow_cascade_viewer"], false));
        self.fields.bool("graphics.debug.gbuffer_viewer", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "gbuffer_viewer"], false));
        self.fields.bool("graphics.debug.gpu_timing", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "gpu_timing"], false));
    }

    fn seed_plugin_fields(&mut self) {
        for tab in &self.plugin_tabs {
            if tab.plugin_id == "newengine.engine" {
                continue;
            }
            for field in &tab.fields {
                let key = plugin_field_key(&tab.plugin_id, &field.path);
                let current = plugin_field_current(&self.config, &tab.plugin_id, field)
                    .unwrap_or_else(|| Value::String(String::new()));
                match field.kind.as_str() {
                    "bool" => {
                        self.fields.bool(&key, current.as_bool().unwrap_or(false));
                    }
                    "select" => {
                        let value = current_to_string(&current);
                        self.fields.select(&key, value);
                    }
                    _ => {
                        self.fields.string(&key, current_to_string(&current));
                    }
                }
            }
        }
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(13, 17, 25))
            .corner_radius(egui::CornerRadius::same(22))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(42, 52, 70)))
            .inner_margin(egui::Margin::symmetric(22, 18))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    icon(ui, IconKind::Logo, 72.0, egui::Color32::from_rgb(108, 181, 255));
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("NewEngine").size(36.0).strong().color(egui::Color32::from_rgb(241, 246, 255)));
                        ui.label(egui::RichText::new("PreStart").size(20.0).color(egui::Color32::from_rgb(83, 159, 255)));
                    });
                    ui.add_space(34.0);
                    ui.separator();
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Version").size(17.0).color(egui::Color32::from_rgb(229, 235, 248)));
                            ui.label(egui::RichText::new(env!("CARGO_PKG_VERSION")).size(18.0).strong().color(egui::Color32::from_rgb(82, 169, 255)));
                            ui.label(egui::RichText::new("Alpha").size(17.0).color(egui::Color32::from_rgb(229, 235, 248)));
                        });
                        ui.label(egui::RichText::new("Next-Gen Game Engine").size(14.0).color(egui::Color32::from_rgb(137, 148, 171)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgb(16, 21, 31))
                            .corner_radius(egui::CornerRadius::same(12))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(39, 49, 66)))
                            .inner_margin(egui::Margin::symmetric(16, 12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    status_dot(ui, true);
                                    ui.label(egui::RichText::new("Ready to Launch").size(18.0).strong().color(ACCENT_GREEN));
                                });
                                ui.add_space(8.0);
                                if beveled_button(ui, IconKind::Folder, "Open Project Folder").clicked() {
                                    self.open_project_folder();
                                }
                            });
                    });
                });
            });
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        nav_button(ui, &mut self.selected_tab, 0, "Engine", "Core settings and startup identity");
        nav_button(ui, &mut self.selected_tab, 1, "Display", "Monitor, resolution, fullscreen");
        nav_button(ui, &mut self.selected_tab, 2, "Graphics", "Renderer, GPU, profile, debug");
        ui.add_space(14.0);
        ui.label(egui::RichText::new("Plugin configuration").size(12.0).color(egui::Color32::from_rgb(126, 137, 158)));
        ui.separator();
        for (index, tab) in self.plugin_tabs.iter().enumerate() {
            if tab.plugin_id == "newengine.engine" {
                continue;
            }
            nav_button(ui, &mut self.selected_tab, 3 + index, &tab.title, &tab.category);
        }
    }

    fn render_engine_tab(&mut self, ui: &mut egui::Ui) {
        section_header(ui, "Engine", "Host-owned launch roots. This tab writes to the root `engine` block, not `plugins.*`.");
        card(ui, |ui| {
            text_row(ui, "Modules directory", self.fields.string("engine.modules_dir", "plugins"));
            text_row(ui, "Cache files", self.fields.string("engine.cache_files", "cache"));
            text_row(ui, "Config directory", self.fields.string("engine.config", "config"));
        });
    }

    fn render_display_tab(&mut self, ui: &mut egui::Ui) {
        section_header(ui, "Display", "Monitor, resolution and screen mode. Fullscreen is a quick control that maps to the window-mode enum.");
        card(ui, |ui| {
            text_row(ui, "Title", self.fields.string("display.title", "Kayla's Editor"));
            ui.horizontal(|ui| {
                ui.label(label("Resolution"));
                styled_text_edit(ui, self.fields.string("display.width", "1600"), 110.0, 28.0);
                ui.label(egui::RichText::new("×").color(TEXT_MUTED));
                styled_text_edit(ui, self.fields.string("display.height", "900"), 110.0, 28.0);
            });
            text_row(ui, "Monitor", self.fields.string("display.monitor", "primary"));
            select_row(ui, "Window mode", self.fields.select("display.window_mode", "windowed"), &[
                ("windowed", "Windowed"),
                ("borderless", "Borderless fullscreen"),
                ("exclusive_fullscreen", "Exclusive fullscreen"),
            ]);
            ui.horizontal_wrapped(|ui| {
                ui.label(label("Quick controls"));
                switch_chip(ui, "Fullscreen", self.fields.bool("display.fullscreen", false));
                switch_chip(ui, "Borderless", self.fields.bool("display.borderless_fullscreen", false));
                switch_chip(ui, "VSync", self.fields.bool("display.vsync", true));
            });
            text_row(ui, "Refresh rate", self.fields.string("display.refresh_rate", "auto"));
            text_row(ui, "Render scale", self.fields.string("display.render_scale", "1.0"));
            select_row(ui, "HDR", self.fields.select("display.hdr", "auto"), &[
                ("auto", "Auto"),
                ("enabled", "Enabled"),
                ("disabled", "Disabled"),
            ]);
        });
    }

    fn render_graphics_tab(&mut self, ui: &mut egui::Ui) {
        section_header(ui, "Graphics Backend", "Human-facing options map to internal capability and profile IDs after save.");
        card(ui, |ui| {
            select_row(ui, "Renderer", self.fields.select("graphics.renderer_backend", "auto"), &[
                ("auto", "Auto"),
                ("vulkan", "Vulkan"),
                ("null", "NullRenderer"),
                ("dx12", "Future DX12"),
            ]);
            text_row(ui, "GPU / device", self.fields.string("graphics.gpu_device", "auto"));
            select_row(ui, "GPU profile", self.fields.select("graphics.graphics_profile", "auto"), &[
                ("auto", "Auto"),
                ("safe_mode", "Safe Mode"),
                ("legacy_gpu", "Legacy GPU"),
                ("modern_gpu", "Modern GPU"),
                ("rtx", "RTX / Raytracing-capable"),
                ("developer_diagnostics", "Developer Diagnostics"),
            ]);
            select_row(ui, "Shader cache", self.fields.select("graphics.shader_cache_mode", "auto"), &[
                ("auto", "Auto"),
                ("enabled", "Enabled"),
                ("disabled", "Disabled"),
                ("rebuild", "Rebuild now"),
            ]);
        });
        ui.add_space(10.0);
        section_header(ui, "Debug renderer tools", "Optional diagnostics exposed as explicit startup configuration.");
        card(ui, |ui| {
            switch_row(ui, "Enable debug renderer tools", "Expose renderer diagnostics in the launch profile.", self.fields.bool("graphics.debug.enabled", false));
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                switch_chip(ui, "RenderDoc capture", self.fields.bool("graphics.debug.renderdoc_capture", false));
                switch_chip(ui, "Phase viewer", self.fields.bool("graphics.debug.phase_viewer", false));
                switch_chip(ui, "Target viewer", self.fields.bool("graphics.debug.target_viewer", false));
                switch_chip(ui, "Shadow cascade viewer", self.fields.bool("graphics.debug.shadow_cascade_viewer", false));
                switch_chip(ui, "GBuffer viewer", self.fields.bool("graphics.debug.gbuffer_viewer", false));
                switch_chip(ui, "GPU timing", self.fields.bool("graphics.debug.gpu_timing", false));
            });
        });
    }

    fn render_plugin_tab(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(tab) = self.plugin_tabs.get(index).cloned() else {
            section_header(ui, "Plugins", "No plugin configuration tab selected.");
            return;
        };
        if tab.plugin_id == "newengine.engine" {
            self.render_engine_tab(ui);
            return;
        }
        section_header(ui, &tab.title, &format!("{} · {}", tab.plugin_id, tab.source));
        card(ui, |ui| {
            if tab.fields.is_empty() {
                ui.label(egui::RichText::new("This plugin published a tab but no editable startup fields.").color(egui::Color32::from_rgb(164, 174, 195)));
                return;
            }
            for field in &tab.fields {
                self.render_schema_field(ui, &tab.plugin_id, field);
            }
        });
    }

    fn render_schema_field(&mut self, ui: &mut egui::Ui, plugin_id: &str, field: &SchemaField) {
        let key = plugin_field_key(plugin_id, &field.path);
        match field.kind.as_str() {
            "bool" => {
                ui.horizontal(|ui| {
                    let label_width = if ui.available_width() < 330.0 { 94.0 } else { 128.0 };
                    ui.add_sized([label_width, 22.0], egui::Label::new(label(&field.label)));
                    switch_only(ui, self.fields.bool(&key, false));
                    if let Some(default_label) = &field.default_label {
                        ui.label(egui::RichText::new(format!("default: {default_label}")).size(11.0).color(TEXT_MUTED));
                    }
                });
            }
            "select" => {
                ui.horizontal(|ui| {
                    let label_width = if ui.available_width() < 330.0 { 94.0 } else { 128.0 };
                    ui.add_sized([label_width, 22.0], egui::Label::new(label(&field.label)));
                    let selected = self.fields.select(&key, field.options.first().map(|o| o.value.as_str()).unwrap_or(""));
                    let width = (ui.available_width() - 4.0).clamp(120.0, 260.0);
                    styled_select_dynamic(ui, key.clone(), selected, &field.options, width);
                    if let Some(default_label) = &field.default_label {
                        ui.label(egui::RichText::new(format!("default: {default_label}")).size(11.0).color(TEXT_MUTED));
                    }
                });
            }
            _ => {
                let editor = self.fields.string(&key, "");
                text_row(ui, &field.label, editor);
            }
        }
    }


    fn render_dashboard(&mut self, ui: &mut egui::Ui) {
        if let Some(warning) = &self.parse_warning {
            ui.colored_label(egui::Color32::from_rgb(255, 184, 112), warning);
            ui.add_space(8.0);
        }

        let available_width = ui.available_width();
        let gap = 14.0;
        let right_width = (available_width * 0.31).clamp(360.0, 430.0);
        let left_width = (available_width - right_width - gap).max(620.0);

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(left_width);
                self.render_left_launch_panel(ui);
            });
            ui.add_space(gap);
            ui.vertical(|ui| {
                ui.set_width(right_width);
                self.render_right_modules_panel(ui);
            });
        });
    }

    fn render_left_launch_panel(&mut self, ui: &mut egui::Ui) {
        launcher_card(ui, |ui| {
            card_title(ui, IconKind::Project, "PROJECT / PROFILE", None);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                icon_box(ui, IconKind::Project);
                ui.vertical(|ui| {
                    styled_text_edit(ui, self.fields.string("project.name", "MyGameProject"), (ui.available_width() - 52.0).max(160.0), 30.0);
                    styled_text_edit(ui, self.fields.string("project.path", "."), (ui.available_width() - 52.0).max(160.0), 26.0);
                });
                icon_button_box(ui, IconKind::Settings);
            });
            ui.add_space(14.0);

            card_title(ui, IconKind::Terminal, "LAUNCH PARAMETERS", None);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                styled_text_edit(ui, self.fields.string("launch.parameters", "--windowed --devmode --log --console"), (ui.available_width() - 46.0).max(220.0), 34.0);
                icon_button_box(ui, IconKind::Settings);
            });
            ui.add_space(14.0);

            ui.columns(2, |columns| {
                columns[0].vertical(|ui| {
                    card_title(ui, IconKind::Chip, "RENDERER", None);
                    select_row(ui, "", self.fields.select("graphics.renderer_backend", "auto"), &[
                        ("auto", "Auto"),
                        ("vulkan", "Vulkan"),
                        ("null", "NullRenderer"),
                        ("dx12", "Future DX12"),
                    ]);
                    ui.add_space(10.0);
                    card_title(ui, IconKind::Monitor, "RESOLUTION", None);
                    ui.horizontal(|ui| {
                        styled_text_edit(ui, self.fields.string("display.width", "1600"), 90.0, 30.0);
                        ui.label("×");
                        styled_text_edit(ui, self.fields.string("display.height", "900"), 90.0, 30.0);
                    });
                    ui.add_space(10.0);
                    card_title(ui, IconKind::Check, "VSYNC", None);
                    switch_row(ui, "Enabled", "Synchronize present rate with the selected display refresh.", self.fields.bool("display.vsync", true));
                });
                columns[1].vertical(|ui| {
                    card_title(ui, IconKind::ScreenMode, "SCREEN MODE", None);
                    segmented_screen_mode(ui, self.fields.select("display.window_mode", "windowed"));
                    ui.add_space(10.0);
                    card_title(ui, IconKind::Check, "FULLSCREEN", None);
                    switch_row(ui, "Fullscreen", "Quick toggle between windowed and fullscreen-oriented presentation.", self.fields.bool("display.fullscreen", false));
                    switch_row(ui, "Borderless fullscreen", "Use borderless fullscreen presentation instead of a bordered window.", self.fields.bool("display.borderless_fullscreen", false));
                    ui.add_space(12.0);
                    card_title(ui, IconKind::ScreenMode, "STARTUP SCENE / PROFILE", None);
                    ui.horizontal(|ui| {
                        styled_text_edit(ui, self.fields.string("launch.startup_scene", "MainMenu"), (ui.available_width() - 44.0).max(180.0), 32.0);
                        icon_button_box(ui, IconKind::Settings);
                    });
                });
            });
        });

        ui.add_space(12.0);
        launcher_card(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, IconKind::Bookmark, 26.0, egui::Color32::from_rgb(143, 164, 196));
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("REMEMBER LAST OPTIONS").size(13.0).strong().color(egui::Color32::from_rgb(162, 178, 203)));
                    ui.label(egui::RichText::new("Load the last used configuration on startup").size(12.0).color(egui::Color32::from_rgb(142, 154, 178)));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    switch_only(ui, self.fields.bool("launch.remember_last_options", true));
                });
            });
        });
    }

    fn render_right_modules_panel(&mut self, ui: &mut egui::Ui) {
        self.render_plugins_modules_card(ui);
        ui.add_space(12.0);
        self.render_selected_plugin_config_card(ui);
        ui.add_space(12.0);
        self.render_recent_configs_card(ui);
    }

    fn render_plugins_modules_card(&mut self, ui: &mut egui::Ui) {
        launcher_card(ui, |ui| {
            let enabled_count = self.plugin_tabs.iter().filter(|tab| tab.plugin_id != "newengine.engine" && tab.enabled).count();
            ui.horizontal(|ui| {
                card_title(ui, IconKind::Puzzle, "PLUGINS / MODULES", Some(&format!("{enabled_count} enabled")));
            });
            ui.add_space(8.0);
            for index in 0..self.plugin_tabs.len() {
                if self.plugin_tabs[index].plugin_id == "newengine.engine" {
                    continue;
                }
                let selected = self.selected_plugin.as_deref() == Some(self.plugin_tabs[index].plugin_id.as_str());
                let clicked = plugin_module_entry(ui, &self.plugin_tabs[index], selected);
                if clicked {
                    self.toggle_plugin(index);
                }
            }
            ui.add_space(8.0);
            let _response = beveled_button(ui, IconKind::Settings, "Manage Plugins...");
        });
    }

    fn toggle_plugin(&mut self, index: usize) {
        let Some(tab) = self.plugin_tabs.get_mut(index) else { return; };
        tab.enabled = !tab.enabled;
        if tab.enabled {
            self.selected_plugin = Some(tab.plugin_id.clone());
            self.status = format!("Enabled {}. Plugin configuration is visible.", tab.title);
        } else {
            if self.selected_plugin.as_deref() == Some(tab.plugin_id.as_str()) {
                self.selected_plugin = None;
            }
            self.status = format!("Disabled {}. Its configuration block is hidden.", tab.title);
        }
    }

    fn render_selected_plugin_config_card(&mut self, ui: &mut egui::Ui) {
        let Some(selected_id) = self.selected_plugin.clone() else {
            return;
        };
        let Some(tab) = self.plugin_tabs.iter().find(|tab| tab.plugin_id == selected_id && tab.enabled).cloned() else {
            return;
        };
        launcher_card(ui, |ui| {
            card_title(ui, plugin_icon(&tab), &format!("{} CONFIG", tab.title.to_uppercase()), None);
            ui.label(egui::RichText::new(format!("{} · {}", tab.plugin_id, tab.category)).size(11.0).color(egui::Color32::from_rgb(120, 132, 154)));
            ui.add_space(8.0);
            if tab.fields.is_empty() {
                ui.label(egui::RichText::new("No editable startup fields were published by this plugin.").color(egui::Color32::from_rgb(150, 162, 185)));
            } else {
                egui::ScrollArea::vertical()
                    .id_salt(format!("plugin-config-scroll-{}", tab.plugin_id))
                    .max_height(176.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for field in &tab.fields {
                            self.render_schema_field(ui, &tab.plugin_id, field);
                        }
                    });
            }
        });
    }

    fn render_recent_configs_card(&mut self, ui: &mut egui::Ui) {
        launcher_card(ui, |ui| {
            card_title(ui, IconKind::Clock, "RECENT CONFIGURATIONS", None);
            ui.add_space(8.0);
            for (title, meta) in [
                ("Current config", "canonical config.json"),
                ("Developer diagnostics", "renderer debug preset"),
            ] {
                ui.horizontal(|ui| {
                    icon(ui, IconKind::Clock, 18.0, egui::Color32::from_rgb(132, 149, 176));
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(title).size(13.0).color(egui::Color32::from_rgb(214, 222, 238)));
                        ui.label(egui::RichText::new(meta).size(10.5).color(egui::Color32::from_rgb(116, 128, 151)));
                    });
                });
                ui.separator();
            }
        });
    }

    fn save(&mut self) -> Result<(), String> {
        let mut config = self.config.clone();
        self.apply_engine(&mut config);
        self.apply_display(&mut config);
        self.apply_graphics(&mut config);
        self.apply_launch_options(&mut config);
        self.apply_plugin_tabs(&mut config);
        self.apply_plugin_enabled_states(&mut config);
        let text = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("config serialization failed: {e}"))?;
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("config directory create failed path='{}' err={e}", parent.display()))?;
        }
        fs::write(&self.config_path, format!("{text}\n"))
            .map_err(|e| format!("config write failed path='{}' err={e}", self.config_path.display()))?;
        self.config = config;
        self.status = format!("Saved {}", self.config_path.display());
        Ok(())
    }

    fn apply_engine(&self, config: &mut Value) {
        set_segments(config, &["engine", "modules_dir"], Value::String(self.fields.string_value("engine.modules_dir", "plugins")));
        set_segments(config, &["engine", "cache_files"], Value::String(self.fields.string_value("engine.cache_files", "cache")));
        set_segments(config, &["engine", "config"], Value::String(self.fields.string_value("engine.config", "config")));
    }

    fn apply_launch_options(&self, config: &mut Value) {
        set_segments(config, &["plugins", "newengine", "startup_window", "project", "name"], Value::String(self.fields.string_value("project.name", "MyGameProject")));
        set_segments(config, &["plugins", "newengine", "startup_window", "project", "path"], Value::String(self.fields.string_value("project.path", ".")));
        set_segments(config, &["plugins", "newengine", "startup_window", "launch_parameters"], Value::String(self.fields.string_value("launch.parameters", "")));
        set_segments(config, &["plugins", "newengine", "startup_window", "startup_scene"], Value::String(self.fields.string_value("launch.startup_scene", "MainMenu")));
        set_segments(config, &["plugins", "newengine", "startup_window", "remember_last_options"], Value::Bool(self.fields.bool_value("launch.remember_last_options", true)));
    }

    fn apply_display(&self, config: &mut Value) {
        let title = self.fields.string_value("display.title", "Kayla's Editor");
        let width = parse_i64(&self.fields.string_value("display.width", "1600"), 1600).max(320);
        let height = parse_i64(&self.fields.string_value("display.height", "900"), 900).max(240);
        let mut mode = self.fields.select_value("display.window_mode", "windowed");
        let fullscreen = self.fields.bool_value("display.fullscreen", false);
        if fullscreen && mode == "windowed" {
            mode = "borderless".to_owned();
        }
        let borderless = self.fields.bool_value("display.borderless_fullscreen", false) || mode == "borderless";
        let vsync = self.fields.bool_value("display.vsync", true);
        let monitor = self.fields.string_value("display.monitor", "primary");
        let refresh_rate = self.fields.string_value("display.refresh_rate", "auto");
        let render_scale = parse_f64(&self.fields.string_value("display.render_scale", "1.0"), 1.0).clamp(0.25, 4.0);
        let hdr = self.fields.select_value("display.hdr", "auto");

        set_segments(config, &["window", "title"], Value::String(title.clone()));
        set_segments(config, &["window", "width"], Value::Number(width.into()));
        set_segments(config, &["window", "height"], Value::Number(height.into()));
        set_segments(config, &["plugins", "newengine", "platform.winit", "title"], Value::String(title));
        set_segments(config, &["plugins", "newengine", "platform.winit", "width"], Value::Number(width.into()));
        set_segments(config, &["plugins", "newengine", "platform.winit", "height"], Value::Number(height.into()));

        set_segments(config, &["plugins", "newengine", "startup_window", "display", "monitor"], Value::String(monitor.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "resolution", "width"], Value::Number(width.into()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "resolution", "height"], Value::Number(height.into()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "window_mode"], Value::String(mode.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "fullscreen"], Value::Bool(fullscreen));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "borderless_fullscreen"], Value::Bool(borderless));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "vsync"], Value::Bool(vsync));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "refresh_rate"], Value::String(refresh_rate.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "render_scale"], number_value(render_scale));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "hdr"], Value::String(hdr.clone()));

        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "monitor"], Value::String(monitor));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "resolution", "width"], Value::Number(width.into()));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "resolution", "height"], Value::Number(height.into()));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "window_mode"], Value::String(mode));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "fullscreen"], Value::Bool(fullscreen));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "borderless_fullscreen"], Value::Bool(borderless));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "vsync"], Value::Bool(vsync));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "refresh_rate"], Value::String(refresh_rate));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "render_scale"], number_value(render_scale));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "hdr"], Value::String(hdr));
    }

    fn apply_graphics(&self, config: &mut Value) {
        let backend = self.fields.select_value("graphics.renderer_backend", "auto");
        let gpu_device = self.fields.string_value("graphics.gpu_device", "auto");
        let profile = self.fields.select_value("graphics.graphics_profile", "auto");
        let shader_cache = self.fields.select_value("graphics.shader_cache_mode", "auto");
        let profile_id = graphics_profile_id(&profile);
        let gpu_safe = profile == "safe_mode";

        set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "renderer_backend"], Value::String(backend.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "gpu_device"], Value::String(gpu_device.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "graphics_profile"], Value::String(profile.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "shader_cache_mode"], Value::String(shader_cache.clone()));

        set_segments(config, &["plugins", "newengine", "renderer", "selection", "backend"], Value::String(backend));
        set_segments(config, &["plugins", "newengine", "renderer", "selection", "gpu_device"], Value::String(gpu_device));
        set_segments(config, &["plugins", "newengine", "renderer", "selection", "graphics_profile"], Value::String(profile.clone()));
        set_segments(config, &["plugins", "newengine", "renderer", "selection", "graphics_profile_id"], Value::String(profile_id.to_owned()));
        set_segments(config, &["plugins", "newengine", "renderer", "selection", "shader_cache_mode"], Value::String(shader_cache.clone()));
        set_segments(config, &["plugins", "newengine", "renderer", "vulkan", "shader_cache", "mode"], Value::String(shader_cache));
        set_segments(config, &["plugins", "newengine", "engine_runtime", "render", "runtime_profile", "id"], Value::String(profile_id.to_owned()));
        set_segments(config, &["plugins", "newengine", "engine_runtime", "render", "runtime_profile", "gpu_safe"], Value::Bool(gpu_safe));

        for (key, path) in [
            ("graphics.debug.enabled", "enabled"),
            ("graphics.debug.renderdoc_capture", "renderdoc_capture"),
            ("graphics.debug.phase_viewer", "phase_viewer"),
            ("graphics.debug.target_viewer", "target_viewer"),
            ("graphics.debug.shadow_cascade_viewer", "shadow_cascade_viewer"),
            ("graphics.debug.gbuffer_viewer", "gbuffer_viewer"),
            ("graphics.debug.gpu_timing", "gpu_timing"),
        ] {
            let value = self.fields.bool_value(key, false);
            set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", path], Value::Bool(value));
            set_segments(config, &["plugins", "newengine", "renderer", "selection", "debug_renderer_tools", path], Value::Bool(value));
            set_segments(config, &["plugins", "newengine", "renderer", "vulkan", "debug_tools", path], Value::Bool(value));
        }
    }

    fn apply_plugin_tabs(&self, config: &mut Value) {
        for tab in &self.plugin_tabs {
            if tab.plugin_id == "newengine.engine" {
                continue;
            }
            for field in &tab.fields {
                let key = plugin_field_key(&tab.plugin_id, &field.path);
                let value = match field.kind.as_str() {
                    "bool" => Value::Bool(self.fields.bool_value(&key, false)),
                    "integer" => Value::Number(parse_i64(&self.fields.string_value(&key, "0"), 0).into()),
                    "number" => number_value(parse_f64(&self.fields.string_value(&key, "0"), 0.0)),
                    "select" => Value::String(self.fields.select_value(&key, "")),
                    "array" | "object" => parse_json_or_string(&self.fields.string_value(&key, "")),
                    _ => Value::String(self.fields.string_value(&key, "")),
                };
                set_plugin_field(config, &tab.plugin_id, &field.path, value);
            }
        }
    }

    fn apply_plugin_enabled_states(&self, config: &mut Value) {
        let mut raw_tabs = get_segments(config, &["plugins", "newengine", "startup_window", "plugin_tabs"])
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        for item in &mut raw_tabs {
            let Some(plugin_id) = item.get("plugin_id").and_then(Value::as_str).map(ToOwned::to_owned) else {
                continue;
            };
            let Some(tab) = self.plugin_tabs.iter().find(|tab| tab.plugin_id == plugin_id) else {
                continue;
            };
            if let Some(object) = item.as_object_mut() {
                object.insert("enabled".to_owned(), Value::Bool(tab.enabled));
            }
        }

        if !raw_tabs.is_empty() {
            set_segments(config, &["plugins", "newengine", "startup_window", "plugin_tabs"], Value::Array(raw_tabs));
        }
    }
}

impl eframe::App for PreStartApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_style(ctx);
        self.center_window_on_startup(ctx);

        egui::TopBottomPanel::top("prestart_header")
            .exact_height(146.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(3, 5, 9)).inner_margin(egui::Margin::symmetric(14, 8)))
            .show(ctx, |ui| self.render_header(ui));

        egui::TopBottomPanel::bottom("prestart_footer")
            .exact_height(78.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(6, 8, 13)).inner_margin(egui::Margin::symmetric(18, 10)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    icon(ui, IconKind::Settings, 22.0, egui::Color32::from_rgb(144, 160, 186));
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("CONFIG PATH").size(10.5).strong().color(egui::Color32::from_rgb(147, 158, 181)));
                        ui.label(egui::RichText::new(self.config_path.display().to_string()).size(12.0).color(egui::Color32::from_rgb(85, 161, 255)));
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("ACTIVE PROFILE").size(10.5).strong().color(egui::Color32::from_rgb(147, 158, 181)));
                        ui.label(egui::RichText::new(self.fields.select_value("graphics.graphics_profile", "auto")).size(12.0).color(egui::Color32::from_rgb(210, 219, 238)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let launch = action_button(ui, IconKind::Launch, "LAUNCH ENGINE", 220.0, true);
                        if launch.clicked() {
                            match self.save() {
                                Ok(()) => {
                                    self.set_outcome(WindowOutcome::LaunchRequested);
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                Err(err) => self.status = err,
                            }
                        }
                        if action_button(ui, IconKind::Save, "SAVE", 136.0, false).clicked() {
                            if let Err(err) = self.save() {
                                self.status = err;
                            }
                        }
                        if action_button(ui, IconKind::Cancel, "CANCEL", 136.0, false).clicked() {
                            self.set_outcome(WindowOutcome::Cancelled);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(8, 11, 17)).inner_margin(egui::Margin::symmetric(18, 14)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("prestart-dashboard-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.render_dashboard(ui));
            });
    }
}

fn resolve_project_folder(config_path: &Path, raw_path: &str) -> PathBuf {
    let trimmed = raw_path.trim();
    let path = if trimmed.is_empty() { PathBuf::from(".") } else { PathBuf::from(trimmed) };
    if path.is_absolute() {
        path
    } else if let Some(parent) = config_path.parent() {
        parent.join(path)
    } else {
        path
    }
}

fn open_folder_in_shell(path: &Path) -> Result<(), String> {
    let target = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(&target);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&target);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&target);
        command
    };

    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        let _ = target;
        return Err("opening folders is not supported on this platform".to_owned());
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("failed to spawn platform folder opener: {err}"))
}

fn status_dot(ui: &mut egui::Ui, enabled: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
    let color = if enabled { ACCENT_GREEN } else { egui::Color32::from_rgb(231, 79, 86) };
    ui.painter().circle_filled(rect.center(), 4.5, color);
    ui.painter().circle_filled(rect.center(), 8.0, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28));
}

fn action_button(ui: &mut egui::Ui, kind: IconKind, text: &str, width: f32, primary: bool) -> egui::Response {
    let fill = if primary {
        egui::Color32::from_rgb(26, 88, 185)
    } else {
        egui::Color32::from_rgb(18, 23, 32)
    };
    let stroke = if primary {
        egui::Stroke::new(1.1, egui::Color32::from_rgb(85, 170, 255))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 53, 72))
    };
    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(stroke)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_min_width(width - 32.0);
            ui.horizontal(|ui| {
                icon(ui, kind, 20.0, if primary { egui::Color32::WHITE } else { egui::Color32::from_rgb(174, 187, 210) });
                ui.add_space(8.0);
                ui.label(egui::RichText::new(text).size(if primary { 16.0 } else { 13.5 }).strong().color(if primary { egui::Color32::WHITE } else { TEXT_PRIMARY }));
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(9), egui::Stroke::new(1.2, ACCENT_BLUE_BRIGHT), egui::StrokeKind::Inside);
    }
    response
}

fn nav_button(ui: &mut egui::Ui, selected: &mut usize, index: usize, title: &str, subtitle: &str) {
    let is_selected = *selected == index;
    let fill = if is_selected {
        egui::Color32::from_rgb(35, 48, 74)
    } else {
        egui::Color32::from_rgb(17, 20, 28)
    };
    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(200.0);
            ui.label(egui::RichText::new(title).size(15.0).strong().color(egui::Color32::from_rgb(224, 232, 248)));
            ui.label(egui::RichText::new(subtitle).size(11.0).color(egui::Color32::from_rgb(128, 139, 160)));
        })
        .response;
    if response.interact(egui::Sense::click()).clicked() {
        *selected = index;
    }
    ui.add_space(6.0);
}

fn section_header(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.label(egui::RichText::new(title).size(24.0).strong().color(egui::Color32::from_rgb(235, 240, 252)));
    ui.label(egui::RichText::new(detail).size(13.0).color(egui::Color32::from_rgb(146, 158, 184)));
    ui.add_space(14.0);
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(20, 24, 33))
        .corner_radius(egui::CornerRadius::same(16))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 45, 60)))
        .inner_margin(egui::Margin::same(16))
        .show(ui, add);
}

fn label(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(13.0).color(egui::Color32::from_rgb(176, 187, 208))
}

fn text_row(ui: &mut egui::Ui, title: &str, value: &mut String) {
    ui.horizontal(|ui| {
        let label_width = if ui.available_width() < 330.0 { 94.0 } else { 128.0 };
        ui.add_sized([label_width, 22.0], egui::Label::new(label(title)));
        let width = (ui.available_width() - 4.0).clamp(120.0, 360.0);
        styled_text_edit(ui, value, width, 28.0);
    });
}

fn select_row(ui: &mut egui::Ui, title: &str, selected: &mut String, options: &[(&str, &str)]) {
    ui.horizontal(|ui| {
        if !title.is_empty() {
            let label_width = if ui.available_width() < 330.0 { 94.0 } else { 128.0 };
            ui.add_sized([label_width, 22.0], egui::Label::new(label(title)));
        }
        let width = (ui.available_width() - 4.0).clamp(130.0, 300.0);
        styled_select_static(ui, title, selected, options, width);
    });
}

fn styled_text_edit(ui: &mut egui::Ui, value: &mut String, width: f32, height: f32) -> egui::Response {
    let frame_response = egui::Frame::new()
        .fill(egui::Color32::from_rgb(9, 12, 18))
        .corner_radius(egui::CornerRadius::same(7))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70)))
        .inner_margin(egui::Margin::symmetric(10, 3))
        .show(ui, |ui| {
            ui.add_sized(
                [width, height],
                egui::TextEdit::singleline(value)
                    .frame(false)
                    .desired_width(width),
            )
        });
    let response = frame_response.inner;
    let stroke = if response.has_focus() {
        egui::Stroke::new(1.35, ACCENT_BLUE_BRIGHT)
    } else if response.hovered() || frame_response.response.hovered() {
        egui::Stroke::new(1.1, ACCENT_BLUE)
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70))
    };
    ui.painter().rect_stroke(frame_response.response.rect, egui::CornerRadius::same(7), stroke, egui::StrokeKind::Inside);
    response
}

fn styled_multiline_edit(ui: &mut egui::Ui, value: &mut String, width: f32, height: f32) -> egui::Response {
    let frame_response = egui::Frame::new()
        .fill(egui::Color32::from_rgb(9, 12, 18))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70)))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.add_sized(
                [width, height],
                egui::TextEdit::multiline(value)
                    .frame(false)
                    .desired_width(width),
            )
        });
    let response = frame_response.inner;
    let stroke = if response.has_focus() {
        egui::Stroke::new(1.35, ACCENT_BLUE_BRIGHT)
    } else if response.hovered() || frame_response.response.hovered() {
        egui::Stroke::new(1.1, ACCENT_BLUE)
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70))
    };
    ui.painter().rect_stroke(frame_response.response.rect, egui::CornerRadius::same(8), stroke, egui::StrokeKind::Inside);
    response
}

fn styled_select_static(ui: &mut egui::Ui, id_salt: &str, selected: &mut String, options: &[(&str, &str)], width: f32) {
    let current_label = options
        .iter()
        .find(|(value, _)| *value == selected.as_str())
        .map(|(_, label)| (*label).to_owned())
        .unwrap_or_else(|| selected.clone());
    let owned_options: Vec<(String, String)> = options
        .iter()
        .map(|(value, label)| ((*value).to_owned(), (*label).to_owned()))
        .collect();

    styled_select_box(ui, id_salt, selected, &current_label, &owned_options, width);
}

fn styled_select_dynamic(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    selected: &mut String,
    options: &[SelectOption],
    width: f32,
) {
    let current_label = option_label(options, selected.as_str());
    let owned_options: Vec<(String, String)> = options
        .iter()
        .map(|option| (option.value.clone(), option.label.clone()))
        .collect();

    styled_select_box(ui, id_salt, selected, &current_label, &owned_options, width);
}

fn styled_select_box(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    selected: &mut String,
    current_label: &str,
    options: &[(String, String)],
    width: f32,
) {
    let button_id = ui.make_persistent_id(("prestart_select", id_salt));
    let popup_id = button_id.with("popup");
    let area_id = popup_id.with("area");
    let mut open = ui.ctx().data(|data| data.get_temp::<bool>(popup_id).unwrap_or(false));

    let button_height = 34.0;
    let desired_size = egui::vec2(width, button_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.clicked() {
        open = !open;
        ui.ctx().data_mut(|data| data.insert_temp(popup_id, open));
    }

    let fill = if response.hovered() || open {
        egui::Color32::from_rgb(14, 20, 31)
    } else {
        egui::Color32::from_rgb(9, 12, 18)
    };
    let stroke = if open {
        egui::Stroke::new(1.25, ACCENT_BLUE_BRIGHT)
    } else if response.hovered() {
        egui::Stroke::new(1.1, ACCENT_BLUE)
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70))
    };

    ui.painter().rect_filled(rect, egui::CornerRadius::same(8), fill);
    ui.painter().rect_stroke(rect, egui::CornerRadius::same(8), stroke, egui::StrokeKind::Inside);
    ui.painter().rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.right() - 34.0, rect.top() + 1.0),
            egui::pos2(rect.right() - 1.0, rect.bottom() - 1.0),
        ),
        egui::CornerRadius { nw: 0, ne: 8, sw: 0, se: 8 },
        egui::Color32::from_rgb(14, 18, 27),
    );
    ui.painter().vline(
        rect.right() - 34.0,
        (rect.top() + 4.0)..=(rect.bottom() - 4.0),
        egui::Stroke::new(1.0, egui::Color32::from_rgb(32, 42, 58)),
    );
    let label_clip = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.top()),
        egui::pos2(rect.right() - 38.0, rect.bottom()),
    );
    ui.painter().with_clip_rect(label_clip).text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        current_label,
        egui::FontId::proportional(13.5),
        TEXT_PRIMARY,
    );
    paint_chevron(ui, egui::Rect::from_center_size(egui::pos2(rect.right() - 17.0, rect.center().y), egui::vec2(14.0, 14.0)), open);

    let mut popup_rect: Option<egui::Rect> = None;
    if open {
        let popup_pos = egui::pos2(rect.left(), rect.bottom() + 6.0);
        let popup = egui::Area::new(area_id)
            .order(egui::Order::Foreground)
            .fixed_pos(popup_pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(10, 14, 21))
                    .corner_radius(egui::CornerRadius::same(10))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(53, 78, 118)))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_min_width(width);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(240.0)
                            .show(ui, |ui| {
                                for (value, label) in options {
                                    if popup_option_row(ui, label, selected.as_str() == value.as_str()).clicked() {
                                        *selected = value.clone();
                                        open = false;
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                    });
            });
        popup_rect = Some(popup.response.rect);
    }

    if open && ui.ctx().input(|i| i.pointer.any_pressed()) {
        if let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
            let clicked_button = rect.contains(pointer_pos);
            let clicked_popup = popup_rect.map(|r| r.contains(pointer_pos)).unwrap_or(false);
            if !clicked_button && !clicked_popup {
                open = false;
            }
        }
    }

    ui.ctx().data_mut(|data| {
        data.insert_temp(popup_id, open);
    });
}

fn popup_option_row(ui: &mut egui::Ui, label: &str, is_selected: bool) -> egui::Response {
    let desired_size = egui::vec2(ui.available_width(), 30.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let fill = if is_selected {
        egui::Color32::from_rgb(24, 45, 79)
    } else if response.hovered() {
        egui::Color32::from_rgb(18, 27, 41)
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke = if is_selected {
        egui::Stroke::new(1.0, ACCENT_BLUE)
    } else if response.hovered() {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(55, 81, 120))
    } else {
        egui::Stroke::NONE
    };
    ui.painter().rect_filled(rect, egui::CornerRadius::same(7), fill);
    if stroke != egui::Stroke::NONE {
        ui.painter().rect_stroke(rect, egui::CornerRadius::same(7), stroke, egui::StrokeKind::Inside);
    }
    if is_selected {
        ui.painter().circle_filled(egui::pos2(rect.left() + 12.0, rect.center().y), 3.5, ACCENT_BLUE_BRIGHT);
    }
    ui.painter().text(
        egui::pos2(rect.left() + if is_selected { 22.0 } else { 12.0 }, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.8),
        if is_selected { egui::Color32::WHITE } else { TEXT_PRIMARY },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_chevron(ui: &mut egui::Ui, rect: egui::Rect, open: bool) {
    let stroke = egui::Stroke::new(1.7, egui::Color32::from_rgb(198, 210, 232));
    let (a, b, c) = if open {
        (
            egui::pos2(rect.left() + 3.0, rect.bottom() - 4.5),
            egui::pos2(rect.center().x, rect.top() + 4.5),
            egui::pos2(rect.right() - 3.0, rect.bottom() - 4.5),
        )
    } else {
        (
            egui::pos2(rect.left() + 3.0, rect.top() + 4.5),
            egui::pos2(rect.center().x, rect.bottom() - 4.5),
            egui::pos2(rect.right() - 3.0, rect.top() + 4.5),
        )
    };
    ui.painter().line_segment([a, b], stroke);
    ui.painter().line_segment([b, c], stroke);
}

fn launcher_card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(15, 19, 27))
        .corner_radius(egui::CornerRadius::same(14))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(33, 42, 58)))
        .inner_margin(egui::Margin::same(16))
        .show(ui, add);
}

fn card_title(ui: &mut egui::Ui, icon_kind: IconKind, title: &str, pill: Option<&str>) {
    ui.horizontal(|ui| {
        icon(ui, icon_kind, 22.0, egui::Color32::from_rgb(91, 167, 255));
        ui.label(egui::RichText::new(title).size(13.0).strong().color(egui::Color32::from_rgb(91, 167, 255)));
        if let Some(pill) = pill {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(23, 29, 40))
                    .corner_radius(egui::CornerRadius::same(12))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(42, 52, 70)))
                    .inner_margin(egui::Margin::symmetric(12, 4))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(pill).size(12.0).color(egui::Color32::from_rgb(210, 220, 238)));
                    });
            });
        }
    });
}

fn icon_box(ui: &mut egui::Ui, kind: IconKind) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(18, 23, 33))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 53, 72)))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            icon(ui, kind, 24.0, egui::Color32::from_rgb(154, 170, 196));
        });
}

fn switch_only(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let desired_size = egui::vec2(52.0, 28.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    let t = ui.ctx().animate_bool(response.id, *value);
    let track_fill = if *value {
        egui::Color32::from_rgb(32, 86, 170)
    } else {
        egui::Color32::from_rgb(25, 31, 43)
    };
    let track_fill = egui::Color32::from_rgba_unmultiplied(track_fill.r(), track_fill.g(), track_fill.b(), 255);
    let stroke_color = if response.hovered() { ACCENT_BLUE_BRIGHT } else if *value { ACCENT_BLUE } else { egui::Color32::from_rgb(58, 69, 89) };
    let glow_color = if *value { egui::Color32::from_rgba_unmultiplied(74, 150, 255, 28) } else { egui::Color32::from_rgba_unmultiplied(104, 116, 138, 14) };

    ui.painter().rect_filled(rect.expand(1.5), egui::CornerRadius::same(14), glow_color);
    ui.painter().rect_filled(rect, egui::CornerRadius::same(14), track_fill);
    ui.painter().rect_stroke(rect, egui::CornerRadius::same(14), egui::Stroke::new(1.2, stroke_color), egui::StrokeKind::Inside);

    let knob_radius = 10.0;
    let knob_x = egui::lerp((rect.left() + 14.0)..=(rect.right() - 14.0), t);
    let knob_center = egui::pos2(knob_x, rect.center().y);
    if *value {
        ui.painter().circle_filled(knob_center, 13.0, egui::Color32::from_rgba_unmultiplied(84, 163, 255, 38));
    }
    ui.painter().circle_filled(knob_center, knob_radius, egui::Color32::from_rgb(244, 248, 255));
    ui.painter().circle_stroke(knob_center, knob_radius, egui::Stroke::new(1.0, egui::Color32::from_rgb(188, 205, 236)));

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn switch_chip(ui: &mut egui::Ui, label_text: &str, value: &mut bool) -> egui::Response {
    egui::Frame::new()
        .fill(if *value { egui::Color32::from_rgb(17, 32, 58) } else { egui::Color32::from_rgb(14, 18, 26) })
        .corner_radius(egui::CornerRadius::same(10))
        .stroke(egui::Stroke::new(1.0, if *value { ACCENT_BLUE } else { egui::Color32::from_rgb(40, 50, 67) }))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label_text).size(12.5).color(TEXT_PRIMARY));
                switch_only(ui, value);
            });
        })
        .response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn switch_row(ui: &mut egui::Ui, title: &str, subtitle: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(title).size(13.0).strong().color(TEXT_PRIMARY));
            if !subtitle.is_empty() {
                ui.label(egui::RichText::new(subtitle).size(11.0).color(TEXT_MUTED));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            switch_only(ui, value);
        });
    });
}

fn icon_button_box(ui: &mut egui::Ui, kind: IconKind) -> egui::Response {
    let response = egui::Frame::new()
        .fill(egui::Color32::from_rgb(16, 20, 29))
        .corner_radius(egui::CornerRadius::same(7))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 55, 72)))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            icon(ui, kind, 18.0, egui::Color32::from_rgb(170, 184, 207));
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(8), egui::Stroke::new(1.0, egui::Color32::from_rgb(75, 145, 235)), egui::StrokeKind::Inside);
    }
    response
}

fn beveled_button(ui: &mut egui::Ui, kind: IconKind, text: &str) -> egui::Response {
    let response = egui::Frame::new()
        .fill(egui::Color32::from_rgb(20, 25, 34))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 53, 72)))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, kind, 17.0, egui::Color32::from_rgb(164, 179, 205));
                ui.label(egui::RichText::new(text).size(12.5).color(egui::Color32::from_rgb(210, 219, 236)));
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(9), egui::Stroke::new(1.0, egui::Color32::from_rgb(74, 140, 225)), egui::StrokeKind::Inside);
    }
    response
}

fn segmented_screen_mode(ui: &mut egui::Ui, selected: &mut String) {
    ui.horizontal(|ui| {
        for (value, label) in [
            ("windowed", "Windowed"),
            ("borderless", "Borderless"),
            ("exclusive_fullscreen", "Fullscreen"),
        ] {
            let is_selected = selected == value;
            let fill = if is_selected { egui::Color32::from_rgb(28, 56, 98) } else { egui::Color32::from_rgb(17, 21, 30) };
            let stroke = if is_selected { egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 150, 255)) } else { egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 49, 66)) };
            let response = egui::Frame::new()
                .fill(fill)
                .corner_radius(egui::CornerRadius::same(7))
                .stroke(stroke)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(label).size(12.5).color(egui::Color32::from_rgb(218, 227, 246)));
                })
                .response
                .interact(egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.clicked() {
                *selected = value.to_owned();
            }
            if response.hovered() {
                ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(8), egui::Stroke::new(1.0, egui::Color32::from_rgb(82, 160, 255)), egui::StrokeKind::Inside);
            }
        }
    });
}

fn plugin_module_entry(ui: &mut egui::Ui, tab: &PluginTab, selected: bool) -> bool {
    let fill = if selected && tab.enabled {
        egui::Color32::from_rgb(22, 34, 56)
    } else if tab.enabled {
        egui::Color32::from_rgb(12, 17, 26)
    } else {
        egui::Color32::from_rgb(26, 13, 18)
    };
    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(9))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(32, 40, 55)))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, plugin_icon(tab), 20.0, egui::Color32::from_rgb(145, 161, 187));
                let title_width = (ui.available_width() - 96.0).max(90.0);
                ui.add_sized(
                    [title_width, 22.0],
                    egui::Label::new(
                        egui::RichText::new(&tab.title)
                            .size(14.0)
                            .color(egui::Color32::from_rgb(219, 228, 245)),
                    ),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (dot_color, status) = if tab.enabled {
                        (egui::Color32::from_rgb(107, 231, 113), "Enabled")
                    } else {
                        (egui::Color32::from_rgb(231, 79, 86), "Disabled")
                    };
                    ui.label(egui::RichText::new(status).size(12.0).color(egui::Color32::from_rgb(180, 192, 214)));
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 3.5, dot_color);
                    if tab.enabled {
                        ui.painter().circle_filled(rect.center(), 7.0, egui::Color32::from_rgba_unmultiplied(107, 231, 113, 24));
                    } else {
                        ui.painter().circle_filled(rect.center(), 7.0, egui::Color32::from_rgba_unmultiplied(231, 79, 86, 22));
                    }
                });
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.hovered() {
        ui.painter().rect_filled(response.rect.expand(1.0), egui::CornerRadius::same(10), egui::Color32::from_rgba_unmultiplied(82, 158, 255, 26));
        ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(10), egui::Stroke::new(1.2, ACCENT_BLUE_BRIGHT), egui::StrokeKind::Inside);
    }
    response.clicked()
}

fn plugin_icon(tab: &PluginTab) -> IconKind {
    let text = format!("{} {}", tab.title.to_lowercase(), tab.plugin_id.to_lowercase());
    if text.contains("render") || text.contains("vulkan") { IconKind::Renderer }
    else if text.contains("physics") { IconKind::Physics }
    else if text.contains("audio") { IconKind::Audio }
    else if text.contains("input") { IconKind::Input }
    else if text.contains("ui") || text.contains("aurelia") { IconKind::Ui }
    else if text.contains("animation") { IconKind::Animation }
    else if text.contains("script") || text.contains("lua") { IconKind::Script }
    else if text.contains("asset") { IconKind::Folder }
    else { IconKind::Core }
}

fn read_config(path: &Path) -> (Value, Option<String>) {
    match fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => match serde_json::from_str::<Value>(&text) {
            Ok(value) => (value, None),
            Err(err) => (
                Value::Object(Map::new()),
                Some(format!("config.json parse failed: {err}. The editor opened with an empty config; press Cancel to avoid overwriting.")),
            ),
        },
        Ok(_) => (Value::Object(Map::new()), None),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (Value::Object(Map::new()), None),
        Err(err) => (
            Value::Object(Map::new()),
            Some(format!("config.json read failed: {err}. The editor opened with defaults.")),
        ),
    }
}

fn collect_plugin_tabs(config: &Value) -> Vec<PluginTab> {
    let Some(array) = get_segments(config, &["plugins", "newengine", "startup_window", "plugin_tabs"]).and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut tabs = Vec::new();
    for item in array {
        let plugin_id = value_string(item.get("plugin_id"), "unknown.plugin");
        let title = value_string(item.get("title"), &plugin_id);
        let category = value_string(item.get("category"), "Plugin");
        let source = value_string(item.get("source"), "config.json");
        let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let mut fields = Vec::new();
        if let Some(schema_fields) = item
            .get("schema")
            .and_then(|schema| schema.get("fields"))
            .and_then(Value::as_array)
        {
            for raw_field in schema_fields {
                let path = value_string(raw_field.get("path"), "");
                if path.is_empty() {
                    continue;
                }
                let key = value_string(raw_field.get("key"), &path);
                let label_text = value_string(raw_field.get("label"), &key);
                let kind = value_string(raw_field.get("kind"), "string");
                let default_label = raw_field
                    .get("default_label")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                let mut options = Vec::new();
                if let Some(raw_options) = raw_field.get("options").and_then(Value::as_array) {
                    for option in raw_options {
                        let value = value_string(option.get("value"), "");
                        if value.is_empty() {
                            continue;
                        }
                        let label = value_string(option.get("label"), &value);
                        options.push(SelectOption { value, label });
                    }
                }
                fields.push(SchemaField {
                    key,
                    path,
                    label: label_text,
                    kind,
                    options,
                    default_label,
                });
            }
        }
        tabs.push(PluginTab { plugin_id, title, category, source, enabled, fields });
    }
    tabs.sort_by(|a, b| a.title.cmp(&b.title));
    tabs
}

fn plugin_field_current(config: &Value, plugin_id: &str, field: &SchemaField) -> Option<Value> {
    if let Some(current) = plugin_schema_field(config, plugin_id, &field.path, "current") {
        return Some(current.clone());
    }
    if let Some(default) = plugin_schema_field(config, plugin_id, &field.path, "default") {
        return Some(default.clone());
    }
    get_plugin_field(config, plugin_id, &field.path).cloned()
}

fn plugin_schema_field<'a>(config: &'a Value, plugin_id: &str, field_path: &str, key: &str) -> Option<&'a Value> {
    let tabs = get_segments(config, &["plugins", "newengine", "startup_window", "plugin_tabs"])?.as_array()?;
    for tab in tabs {
        if tab.get("plugin_id").and_then(Value::as_str) != Some(plugin_id) {
            continue;
        }
        let fields = tab.get("schema")?.get("fields")?.as_array()?;
        for field in fields {
            if field.get("path").and_then(Value::as_str) == Some(field_path) {
                return field.get(key);
            }
        }
    }
    None
}

fn plugin_field_key(plugin_id: &str, field_path: &str) -> String {
    format!("plugin::{plugin_id}::{field_path}")
}

fn get_plugin_field<'a>(config: &'a Value, plugin_id: &str, rel_path: &str) -> Option<&'a Value> {
    let plugins = config.get("plugins")?.as_object()?;
    let mut id_parts = plugin_id.split('.');
    let namespace = id_parts.next()?;
    let tail_parts: Vec<&str> = id_parts.collect();
    let namespace_value = plugins.get(namespace)?;
    if tail_parts.is_empty() {
        return get_path(namespace_value, rel_path);
    }
    let tail_literal = tail_parts.join(".");
    if let Some(root) = namespace_value.get(&tail_literal) {
        return get_path(root, rel_path);
    }
    let mut node = namespace_value;
    for part in tail_parts {
        node = node.get(part)?;
    }
    get_path(node, rel_path)
}

fn set_plugin_field(config: &mut Value, plugin_id: &str, rel_path: &str, value: Value) {
    ensure_object(config);
    let mut id_parts = plugin_id.split('.');
    let Some(namespace) = id_parts.next() else { return; };
    let tail_parts: Vec<&str> = id_parts.collect();
    let plugins = ensure_child_object(config, "plugins");
    let namespace_value = ensure_child_object(plugins, namespace);
    if tail_parts.is_empty() {
        set_path(namespace_value, rel_path, value);
        return;
    }
    let tail_literal = tail_parts.join(".");
    let use_literal = namespace_value
        .as_object()
        .and_then(|object| object.get(&tail_literal))
        .is_some()
        || tail_literal.starts_with("platform.");
    if use_literal {
        let root = ensure_child_object(namespace_value, &tail_literal);
        set_path(root, rel_path, value);
        return;
    }
    let mut node = namespace_value;
    for part in tail_parts {
        node = ensure_child_object(node, part);
    }
    set_path(node, rel_path, value);
}

fn value_string(value: Option<&Value>, default: &str) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(other) => serde_json::to_string(other).unwrap_or_else(|_| default.to_owned()),
        None => default.to_owned(),
    }
}

fn current_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

fn value_string_segments(config: &Value, segments: &[&str], default: &str) -> String {
    value_string(get_segments(config, segments), default)
}

fn value_bool_segments(config: &Value, segments: &[&str], default: bool) -> bool {
    get_segments(config, segments).and_then(Value::as_bool).unwrap_or(default)
}

fn value_i64_segments(config: &Value, segments: &[&str], default: i64) -> i64 {
    get_segments(config, segments).and_then(Value::as_i64).unwrap_or(default)
}

fn value_f64_segments(config: &Value, segments: &[&str], default: f64) -> f64 {
    get_segments(config, segments).and_then(Value::as_f64).unwrap_or(default)
}

fn first_string_segments(config: &Value, paths: &[&[&str]], default: &str) -> String {
    for path in paths {
        if let Some(value) = get_segments(config, path) {
            return value_string(Some(value), default);
        }
    }
    default.to_owned()
}

fn first_i64_segments(config: &Value, paths: &[&[&str]], default: i64) -> i64 {
    for path in paths {
        if let Some(value) = get_segments(config, path).and_then(Value::as_i64) {
            return value;
        }
    }
    default
}

fn get_segments<'a>(root: &'a Value, segments: &[&str]) -> Option<&'a Value> {
    let mut node = root;
    for segment in segments {
        node = node.get(*segment)?;
    }
    Some(node)
}

fn set_segments(root: &mut Value, segments: &[&str], value: Value) {
    if segments.is_empty() {
        *root = value;
        return;
    }
    ensure_object(root);
    let mut node = root;
    for segment in &segments[..segments.len() - 1] {
        node = ensure_child_object(node, segment);
    }
    let leaf = segments[segments.len() - 1];
    ensure_object(node).insert(leaf.to_owned(), value);
}

fn get_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut node = root;
    for part in path.split('.').filter(|part| !part.is_empty()) {
        node = node.get(part)?;
    }
    Some(node)
}

fn set_path(root: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').filter(|part| !part.is_empty()).collect();
    set_segments(root, &parts, value);
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("value forced to object")
}

fn ensure_child_object<'a>(value: &'a mut Value, key: &str) -> &'a mut Value {
    let object = ensure_object(value);
    let child = object.entry(key.to_owned()).or_insert_with(|| Value::Object(Map::new()));
    if !child.is_object() {
        *child = Value::Object(Map::new());
    }
    child
}

fn parse_i64(text: &str, default: i64) -> i64 {
    text.trim().parse::<i64>().unwrap_or(default)
}

fn parse_f64(text: &str, default: f64) -> f64 {
    text.trim().parse::<f64>().unwrap_or(default)
}

fn parse_json_or_string(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(text.to_owned()))
}

fn number_value(value: f64) -> Value {
    Number::from_f64(value).map(Value::Number).unwrap_or(Value::Null)
}

fn graphics_profile_id(profile: &str) -> &'static str {
    match profile {
        "safe_mode" => "newengine.render.runtime.tier.safe",
        "legacy_gpu" => "newengine.render.runtime.tier.legacy",
        "modern_gpu" => "newengine.render.runtime.tier.gtx",
        "rtx" | "rtx_raytracing_capable" => "newengine.render.runtime.tier.rtx",
        "developer_diagnostics" => "newengine.render.runtime.tier.developer_diagnostics",
        _ => "newengine.render.runtime.tier.auto",
    }
}

fn option_label(options: &[SelectOption], selected: &str) -> String {
    options
        .iter()
        .find(|option| option.value == selected)
        .map(|option| option.label.clone())
        .unwrap_or_else(|| selected.to_owned())
}

#[cfg(test)]
mod tests {
    use super::graphics_profile_id;

    #[test]
    fn public_profile_names_are_not_gpu_model_names() {
        assert_eq!(graphics_profile_id("rtx"), "newengine.render.runtime.tier.rtx");
        assert_eq!(graphics_profile_id("legacy_gpu"), "newengine.render.runtime.tier.legacy");
        assert_ne!(graphics_profile_id("legacy_gpu"), "newengine.render.runtime.foundation_gtx750");
    }
}
