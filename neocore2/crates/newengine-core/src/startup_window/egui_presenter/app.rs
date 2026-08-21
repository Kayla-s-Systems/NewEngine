#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;
use newengine_loading_api::bootstrap_ui::north_star_bootstrap_ui_style;

use crate::startup::{StartupConfig, WindowPlacement};

use super::model::{PresenterOutcome, RenderPressure, SettingsPage, StatusKind};
use super::persistence::persist_confirmed_settings;
use super::style::color32;
use super::StartupWindowSelection;
use super::SIDEBAR_WIDTH;
use crate::startup_window::{ShadowQuality, StartupLaunchSettings};

pub(super) struct PreStartGraphicsApp {
    pub(super) config_path: PathBuf,
    pub(super) settings: StartupLaunchSettings,
    pub(super) confirmed_settings: StartupLaunchSettings,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) confirmed_width: u32,
    pub(super) confirmed_height: u32,
    pub(super) centered: bool,
    pub(super) confirmed_centered: bool,
    pub(super) page: SettingsPage,
    pub(super) status: String,
    pub(super) status_kind: StatusKind,
    pub(super) outcome: Arc<Mutex<PresenterOutcome>>,
}

impl PreStartGraphicsApp {
    pub(super) fn new(
        config_path: PathBuf,
        startup: StartupConfig,
        outcome: Arc<Mutex<PresenterOutcome>>,
    ) -> Self {
        let centered = matches!(startup.window_placement, WindowPlacement::Centered { .. });
        Self {
            config_path,
            confirmed_settings: startup.launch_settings.clone(),
            settings: startup.launch_settings,
            width: startup.window_size.0,
            height: startup.window_size.1,
            confirmed_width: startup.window_size.0,
            confirmed_height: startup.window_size.1,
            centered,
            confirmed_centered: centered,
            page: SettingsPage::Display,
            status: "Last confirmed launch profile loaded".to_owned(),
            status_kind: StatusKind::Info,
            outcome,
        }
    }
    pub(super) fn is_dirty(&self) -> bool {
        self.settings != self.confirmed_settings
            || self.width != self.confirmed_width
            || self.height != self.confirmed_height
            || self.centered != self.confirmed_centered
    }
    pub(super) fn render_pressure(&self) -> RenderPressure {
        let graphics = &self.settings.graphics;
        let mut score = 0_u32;
        score += match graphics.msaa_samples {
            8 => 5,
            4 => 3,
            2 => 1,
            _ => 0,
        };
        score += u32::from(graphics.fxaa_enabled);
        score += if graphics.taa_enabled { 3 } else { 0 };
        score += if graphics.ssao_enabled { 2 } else { 0 };
        score += if graphics.ssao_enabled && !graphics.ssao_half_resolution {
            2
        } else {
            0
        };
        score += if graphics.ssao_quality_steps > 24 {
            2
        } else {
            0
        };
        score += u32::from(graphics.bloom_enabled);
        score += if graphics.depth_of_field_enabled {
            2
        } else {
            0
        };
        score += u32::from(graphics.motion_blur_enabled);
        score += u32::from(graphics.sun_rays_enabled);
        score += match graphics.shadow_quality {
            ShadowQuality::Cinematic => 4,
            ShadowQuality::Quality => 2,
            ShadowQuality::Balanced => 1,
            ShadowQuality::Performance | ShadowQuality::Off => 0,
        };
        score += if self.settings.display.render_scale > 1.0 {
            ((self.settings.display.render_scale - 1.0) * 6.0).ceil() as u32
        } else {
            0
        };

        match score {
            0..=3 => RenderPressure::Low,
            4..=8 => RenderPressure::Balanced,
            9..=14 => RenderPressure::High,
            _ => RenderPressure::Extreme,
        }
    }
    pub(super) fn set_status(&mut self, kind: StatusKind, message: impl Into<String>) {
        self.status_kind = kind;
        self.status = message.into();
    }
    pub(super) fn reset_defaults(&mut self) {
        self.settings = StartupLaunchSettings::default();
        self.width = 1600;
        self.height = 900;
        self.centered = true;
        self.set_status(
            StatusKind::Warning,
            "Defaults restored in the workbench; press Launch Engine to persist them",
        );
    }
    pub(super) fn launch(&mut self, ctx: &egui::Context) {
        self.settings.display.center_window = self.centered;
        self.settings.normalize();
        self.centered = self.settings.display.center_window;

        let placement = if self.centered {
            WindowPlacement::Centered { offset: (0, 0) }
        } else {
            WindowPlacement::Default
        };
        let selection = StartupWindowSelection {
            launch_settings: self.settings.clone(),
            window_size: (self.width, self.height),
            window_placement: placement,
        };

        match persist_confirmed_settings(&self.config_path, &selection) {
            Ok(()) => {
                if let Ok(mut outcome) = self.outcome.lock() {
                    *outcome = PresenterOutcome::Confirmed(selection);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(err) => {
                self.set_status(StatusKind::Error, format!("Could not save settings: {err}"));
            }
        }
    }
    pub(super) fn cancel(&mut self, ctx: &egui::Context) {
        if let Ok(mut outcome) = self.outcome.lock() {
            *outcome = PresenterOutcome::Cancelled;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for PreStartGraphicsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.cancel(ctx);
            return;
        }
        if ctx.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::Enter)) {
            self.launch(ctx);
            return;
        }

        let style = north_star_bootstrap_ui_style();

        egui::TopBottomPanel::top("newengine_prestart_header")
            .exact_height(84.0)
            .frame(
                egui::Frame::none()
                    .fill(color32(style.palette.bg_deep))
                    .stroke(egui::Stroke::new(1.0, color32(style.palette.edge_soft)))
                    .inner_margin(egui::Margin::symmetric(20.0, 12.0)),
            )
            .show(ctx, |ui| self.show_header(ui));

        egui::TopBottomPanel::bottom("newengine_prestart_footer")
            .exact_height(68.0)
            .frame(
                egui::Frame::none()
                    .fill(color32(style.palette.panel))
                    .stroke(egui::Stroke::new(1.0, color32(style.palette.edge)))
                    .inner_margin(egui::Margin::symmetric(18.0, 12.0)),
            )
            .show(ctx, |ui| self.show_footer(ui, ctx));

        egui::SidePanel::left("newengine_prestart_sidebar")
            .exact_width(SIDEBAR_WIDTH)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(color32(style.palette.bg_deep))
                    .stroke(egui::Stroke::new(1.0, color32(style.palette.edge_soft)))
                    .inner_margin(egui::Margin::same(14.0)),
            )
            .show(ctx, |ui| self.show_sidebar(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(color32(style.palette.bg))
                    .inner_margin(egui::Margin::symmetric(22.0, 18.0)),
            )
            .show(ctx, |ui| {
                self.show_page_header(ui);
                egui::ScrollArea::vertical()
                    .id_salt("newengine_prestart_content")
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        SettingsPage::Display => self.show_display(ui),
                        SettingsPage::Quality => self.show_quality(ui),
                        SettingsPage::Effects => self.show_effects(ui),
                        SettingsPage::Advanced => self.show_advanced(ui),
                    });
            });
    }
}
