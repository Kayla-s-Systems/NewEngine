#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;
use newengine_loading_api::bootstrap_ui::north_star_bootstrap_ui_style;

use super::app::PreStartGraphicsApp;
use super::model::SettingsPage;
use super::style::{color32, pressure_color, status_color};
use super::widgets::{
    aa_summary, nav_button, primary_button, secondary_button, sidebar_card, status_pill,
    summary_metric,
};
use super::{APP_SUBTITLE, APP_TAGLINE, APP_TITLE};

impl PreStartGraphicsApp {
    pub(super) fn show_header(&mut self, ui: &mut egui::Ui) {
        let style = north_star_bootstrap_ui_style();
        ui.horizontal(|ui| {
            egui::Frame::none()
                .fill(color32(style.palette.blue))
                .rounding(egui::Rounding::same(10.0))
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("NS")
                            .size(22.0)
                            .strong()
                            .color(color32(style.palette.bg_deep)),
                    );
                });

            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(APP_TITLE)
                        .size(21.0)
                        .strong()
                        .color(color32(style.palette.text)),
                );
                ui.label(
                    egui::RichText::new(APP_SUBTITLE)
                        .size(12.5)
                        .color(color32(style.palette.text_dim)),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                status_pill(ui, "CORE OWNED", color32(style.palette.ok));
                status_pill(
                    ui,
                    &format!("v{}", env!("CARGO_PKG_VERSION")),
                    color32(style.palette.blue_bright),
                );
            });
        });
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(APP_TAGLINE)
                .size(10.5)
                .monospace()
                .color(color32(style.palette.muted)),
        );
    }
    pub(super) fn show_sidebar(&mut self, ui: &mut egui::Ui) {
        let style = north_star_bootstrap_ui_style();
        ui.label(
            egui::RichText::new("LAUNCH WORKBENCH")
                .size(10.5)
                .strong()
                .color(color32(style.palette.muted)),
        );
        ui.add_space(8.0);

        for page in SettingsPage::ALL {
            if nav_button(ui, page, self.page == page).clicked() {
                self.page = page;
            }
            ui.add_space(5.0);
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);

        sidebar_card(ui, "ACTIVE PROFILE", |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(self.settings.graphics.preset.label())
                        .size(18.0)
                        .strong()
                        .color(color32(style.palette.text)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    status_pill(
                        ui,
                        if self.is_dirty() { "EDITED" } else { "SAVED" },
                        if self.is_dirty() {
                            color32(style.palette.warn)
                        } else {
                            color32(style.palette.ok)
                        },
                    );
                });
            });
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} × {}  /  {}",
                    self.width,
                    self.height,
                    self.settings.display.window_mode.label()
                ))
                .size(11.5)
                .color(color32(style.palette.text_dim)),
            );
            ui.label(
                egui::RichText::new(format!(
                    "Scale {:.2}  /  {}",
                    self.settings.display.render_scale,
                    aa_summary(&self.settings)
                ))
                .size(11.5)
                .color(color32(style.palette.text_dim)),
            );
        });

        ui.add_space(10.0);
        let pressure = self.render_pressure();
        sidebar_card(ui, "RENDER PRESSURE", |ui| {
            ui.label(
                egui::RichText::new(pressure.label())
                    .size(18.0)
                    .strong()
                    .color(pressure_color(pressure)),
            );
            ui.label(
                egui::RichText::new(pressure.detail())
                    .size(11.5)
                    .color(color32(style.palette.text_dim)),
            );
        });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.label(
                egui::RichText::new(
                    self.config_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("config.json"),
                )
                .size(10.5)
                .monospace()
                .color(color32(style.palette.muted)),
            );
            ui.label(
                egui::RichText::new("PERSISTENCE TARGET")
                    .size(9.5)
                    .strong()
                    .color(color32(style.palette.muted)),
            );
        });
    }
    pub(super) fn show_page_header(&self, ui: &mut egui::Ui) {
        let style = north_star_bootstrap_ui_style();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(self.page.number())
                    .size(12.0)
                    .monospace()
                    .strong()
                    .color(color32(style.palette.blue_bright)),
            );
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(self.page.title())
                        .size(24.0)
                        .strong()
                        .color(color32(style.palette.text)),
                );
                ui.label(
                    egui::RichText::new(self.page.description())
                        .size(12.5)
                        .color(color32(style.palette.text_dim)),
                );
            });
        });
        ui.add_space(14.0);
    }
    pub(super) fn show_launch_summary(&self, ui: &mut egui::Ui) {
        let style = north_star_bootstrap_ui_style();
        let outer_width = ui.available_width();
        egui::Frame::none()
            .fill(color32(style.palette.panel_active))
            .stroke(egui::Stroke::new(1.0, color32(style.palette.blue)))
            .rounding(egui::Rounding::same(10.0))
            .inner_margin(egui::Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.set_min_width((outer_width - 32.0).max(1.0));
                if ui.available_width() >= 700.0 {
                    ui.columns(5, |columns| {
                        summary_metric(
                            &mut columns[0],
                            "OUTPUT",
                            &format!("{}×{}", self.width, self.height),
                        );
                        summary_metric(
                            &mut columns[1],
                            "MODE",
                            self.settings.display.window_mode.label(),
                        );
                        summary_metric(&mut columns[2], "AA STACK", &aa_summary(&self.settings));
                        summary_metric(
                            &mut columns[3],
                            "PRESET",
                            self.settings.graphics.preset.label(),
                        );
                        summary_metric(&mut columns[4], "PRESSURE", self.render_pressure().label());
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 22.0;
                        summary_metric(ui, "OUTPUT", &format!("{}×{}", self.width, self.height));
                        summary_metric(ui, "MODE", self.settings.display.window_mode.label());
                        summary_metric(ui, "AA STACK", &aa_summary(&self.settings));
                        summary_metric(ui, "PRESET", self.settings.graphics.preset.label());
                        summary_metric(ui, "PRESSURE", self.render_pressure().label());
                    });
                }
            });
        ui.add_space(12.0);
    }
    pub(super) fn show_footer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let style = north_star_bootstrap_ui_style();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("STATUS")
                    .size(9.5)
                    .strong()
                    .color(color32(style.palette.muted)),
            );
            ui.label(
                egui::RichText::new(&self.status)
                    .size(11.5)
                    .color(status_color(self.status_kind)),
            );
            if self.is_dirty() {
                status_pill(ui, "UNSAVED", color32(style.palette.warn));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary_button(ui, "LAUNCH ENGINE").clicked() {
                    self.launch(ctx);
                }
                if secondary_button(ui, "Cancel").clicked() {
                    self.cancel(ctx);
                }
                if secondary_button(ui, "Reset defaults").clicked() {
                    self.reset_defaults();
                }
                ui.label(
                    egui::RichText::new("Ctrl+Enter to launch  /  Esc to cancel")
                        .size(10.0)
                        .color(color32(style.palette.muted)),
                );
            });
        });
    }
}
