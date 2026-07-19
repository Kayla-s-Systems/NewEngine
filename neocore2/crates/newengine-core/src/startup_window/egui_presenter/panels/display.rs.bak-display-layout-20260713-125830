#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;

use crate::startup_window::{StartupHdrMode, StartupWindowMode};

use super::super::app::PreStartGraphicsApp;
use super::super::widgets::{compact_choice_button, engine_toggle, section_card, setting_label};

impl PreStartGraphicsApp {
    pub(in crate::startup_window::egui_presenter) fn show_display(&mut self, ui: &mut egui::Ui) {
        self.show_launch_summary(ui);

        section_card(
            ui,
            "Output Surface",
            "Native window dimensions and monitor placement",
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (label, width, height) in [
                        ("720p", 1280, 720),
                        ("900p", 1600, 900),
                        ("1080p", 1920, 1080),
                        ("1440p", 2560, 1440),
                        ("4K", 3840, 2160),
                    ] {
                        let selected = self.width == width && self.height == height;
                        if compact_choice_button(ui, label, selected).clicked() {
                            self.width = width;
                            self.height = height;
                        }
                    }
                });
                ui.add_space(10.0);
                egui::Grid::new("newengine_prestart_output_surface")
                    .num_columns(2)
                    .spacing([28.0, 11.0])
                    .show(ui, |ui| {
                        setting_label(ui, "Resolution", "Logical window or fullscreen extent");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut self.width)
                                    .range(640..=16384)
                                    .speed(16),
                            );
                            ui.label("×");
                            ui.add(
                                egui::DragValue::new(&mut self.height)
                                    .range(480..=16384)
                                    .speed(9),
                            );
                        });
                        ui.end_row();

                        setting_label(ui, "Monitor", "-1 selects the primary/default display");
                        ui.add(
                            egui::DragValue::new(&mut self.settings.display.monitor_index)
                                .range(-1..=32),
                        );
                        ui.end_row();

                        setting_label(ui, "Window placement", "Applied before platform startup");
                        let windowed = matches!(
                            self.settings.display.window_mode,
                            StartupWindowMode::Windowed
                        );
                        ui.add_enabled(
                            windowed,
                            egui::Checkbox::new(&mut self.centered, "Center on launch"),
                        );
                        ui.end_row();
                    });
            },
        );

        ui.add_space(12.0);
        section_card(
            ui,
            "Presentation Pipeline",
            "Swapchain policy, display transfer and frame pacing",
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    for mode in StartupWindowMode::ALL {
                        let selected = self.settings.display.window_mode == mode;
                        if compact_choice_button(ui, mode.label(), selected).clicked() {
                            self.settings.display.window_mode = mode;
                            if !matches!(mode, StartupWindowMode::Windowed) {
                                self.centered = false;
                            }
                        }
                    }
                });
                ui.add_space(10.0);
                egui::Grid::new("newengine_prestart_presentation")
                    .num_columns(2)
                    .spacing([28.0, 11.0])
                    .show(ui, |ui| {
                        setting_label(
                            ui,
                            "VSync",
                            "Synchronize presentation with the selected display",
                        );
                        engine_toggle(ui, &mut self.settings.display.vsync, "Enabled");
                        ui.end_row();

                        setting_label(
                            ui,
                            "Refresh rate",
                            "0 lets the platform choose automatically",
                        );
                        let mut hz = self.settings.display.refresh_rate_millihz / 1000;
                        if ui
                            .add(egui::DragValue::new(&mut hz).range(0..=1000).suffix(" Hz"))
                            .changed()
                        {
                            self.settings.display.refresh_rate_millihz = hz.saturating_mul(1000);
                        }
                        ui.end_row();

                        setting_label(
                            ui,
                            "Frame limit",
                            "Independent runtime frame cap; 0 is uncapped",
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.settings.display.frame_limit)
                                .range(0..=1000)
                                .suffix(" FPS"),
                        );
                        ui.end_row();

                        setting_label(
                            ui,
                            "HDR output",
                            "Display transfer contract exposed to the platform backend",
                        );
                        egui::ComboBox::from_id_salt("newengine_hdr_mode")
                            .width(210.0)
                            .selected_text(self.settings.display.hdr.label())
                            .show_ui(ui, |ui| {
                                for value in StartupHdrMode::ALL {
                                    ui.selectable_value(
                                        &mut self.settings.display.hdr,
                                        value,
                                        value.label(),
                                    );
                                }
                            });
                        ui.end_row();

                        setting_label(
                            ui,
                            "Render scale",
                            "Internal renderer extent relative to output resolution",
                        );
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Slider::new(
                                    &mut self.settings.display.render_scale,
                                    0.25..=2.0,
                                )
                                .step_by(0.05)
                                .show_value(true),
                            );
                        });
                        ui.end_row();
                    });

                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Quick scale");
                    for value in [0.75_f32, 1.0, 1.25, 1.5] {
                        let selected =
                            (self.settings.display.render_scale - value).abs() < f32::EPSILON;
                        if compact_choice_button(ui, &format!("{value:.2}×"), selected).clicked() {
                            self.settings.display.render_scale = value;
                        }
                    }
                });
            },
        );
    }
}
