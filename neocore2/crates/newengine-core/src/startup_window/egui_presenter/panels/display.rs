#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;

use crate::startup_window::{StartupHdrMode, StartupWindowMode};

use super::super::app::PreStartGraphicsApp;
use super::super::widgets::{
    compact_choice_button, engine_toggle, section_card, setting_block, setting_group_label,
    value_caption,
};

const DISPLAY_TWO_COLUMN_BREAKPOINT: f32 = 640.0;
const NUMERIC_FIELD_WIDTH: f32 = 112.0;

impl PreStartGraphicsApp {
    pub(in crate::startup_window::egui_presenter) fn show_display(&mut self, ui: &mut egui::Ui) {
        self.show_launch_summary(ui);

        section_card(
            ui,
            "Output Surface",
            "Native window dimensions and monitor placement",
            |ui| {
                setting_group_label(ui, "RESOLUTION PRESETS");
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
                ui.add_space(12.0);

                if ui.available_width() >= DISPLAY_TWO_COLUMN_BREAKPOINT {
                    ui.columns(2, |columns| {
                        self.show_resolution_block(&mut columns[0]);
                        self.show_output_target_stack(&mut columns[1]);
                    });
                } else {
                    self.show_resolution_block(ui);
                    ui.add_space(10.0);
                    self.show_output_target_stack(ui);
                }
            },
        );

        ui.add_space(12.0);
        section_card(
            ui,
            "Presentation Pipeline",
            "Swapchain policy, display transfer and frame pacing",
            |ui| {
                setting_group_label(ui, "WINDOW MODE");
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
                ui.add_space(12.0);

                if ui.available_width() >= DISPLAY_TWO_COLUMN_BREAKPOINT {
                    ui.columns(2, |columns| {
                        self.show_frame_pacing_block(&mut columns[0]);
                        self.show_presentation_quality_stack(&mut columns[1]);
                    });
                } else {
                    self.show_frame_pacing_block(ui);
                    ui.add_space(10.0);
                    self.show_presentation_quality_stack(ui);
                }
            },
        );
    }

    fn show_resolution_block(&mut self, ui: &mut egui::Ui) {
        setting_block(
            ui,
            "Resolution",
            "Logical window or fullscreen extent",
            |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        value_caption(ui, "WIDTH");
                        ui.add_sized(
                            [NUMERIC_FIELD_WIDTH, 30.0],
                            egui::DragValue::new(&mut self.width)
                                .range(640..=16384)
                                .speed(16),
                        );
                    });
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new("×").size(16.0));
                    });
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        value_caption(ui, "HEIGHT");
                        ui.add_sized(
                            [NUMERIC_FIELD_WIDTH, 30.0],
                            egui::DragValue::new(&mut self.height)
                                .range(480..=16384)
                                .speed(9),
                        );
                    });
                });
            },
        );
    }

    fn show_output_target_stack(&mut self, ui: &mut egui::Ui) {
        setting_block(
            ui,
            "Display target",
            "Use -1 to select the primary or platform-default monitor",
            |ui| {
                value_caption(ui, "MONITOR INDEX");
                ui.add_sized(
                    [NUMERIC_FIELD_WIDTH, 30.0],
                    egui::DragValue::new(&mut self.settings.display.monitor_index).range(-1..=32),
                );
            },
        );
        ui.add_space(10.0);
        setting_block(
            ui,
            "Window placement",
            "Applied before native platform startup",
            |ui| {
                let windowed = matches!(
                    self.settings.display.window_mode,
                    StartupWindowMode::Windowed
                );
                ui.add_enabled_ui(windowed, |ui| {
                    engine_toggle(ui, &mut self.centered, "Center window on launch");
                });
                if !windowed {
                    ui.label(
                        egui::RichText::new("Placement is controlled by fullscreen mode")
                            .size(10.0)
                            .italics()
                            .weak(),
                    );
                }
            },
        );
    }

    fn show_frame_pacing_block(&mut self, ui: &mut egui::Ui) {
        setting_block(
            ui,
            "Frame pacing",
            "Synchronization, display timing and independent runtime cap",
            |ui| {
                engine_toggle(ui, &mut self.settings.display.vsync, "VSync enabled");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        value_caption(ui, "REFRESH RATE");
                        let mut hz = self.settings.display.refresh_rate_millihz / 1000;
                        if ui
                            .add_sized(
                                [NUMERIC_FIELD_WIDTH, 30.0],
                                egui::DragValue::new(&mut hz).range(0..=1000).suffix(" Hz"),
                            )
                            .changed()
                        {
                            self.settings.display.refresh_rate_millihz = hz.saturating_mul(1000);
                        }
                    });
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        value_caption(ui, "FRAME LIMIT");
                        ui.add_sized(
                            [NUMERIC_FIELD_WIDTH, 30.0],
                            egui::DragValue::new(&mut self.settings.display.frame_limit)
                                .range(0..=1000)
                                .suffix(" FPS"),
                        );
                    });
                });
                ui.add_space(7.0);
                ui.label(
                    egui::RichText::new("0 lets the platform choose or disables the cap")
                        .size(10.0)
                        .weak(),
                );
            },
        );
    }

    fn show_presentation_quality_stack(&mut self, ui: &mut egui::Ui) {
        setting_block(
            ui,
            "HDR output",
            "Display transfer contract exposed to the platform backend",
            |ui| {
                egui::ComboBox::from_id_salt("newengine_hdr_mode")
                    .width(ui.available_width().min(240.0))
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
            },
        );
        ui.add_space(10.0);
        setting_block(
            ui,
            "Render scale",
            "Internal renderer extent relative to output resolution",
            |ui| {
                ui.add(
                    egui::Slider::new(&mut self.settings.display.render_scale, 0.25..=2.0)
                        .step_by(0.05)
                        .show_value(true),
                );
                ui.add_space(7.0);
                value_caption(ui, "QUICK SCALE");
                ui.horizontal_wrapped(|ui| {
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
