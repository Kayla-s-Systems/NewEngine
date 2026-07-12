#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;

use super::super::super::app::PreStartGraphicsApp;
use super::super::super::widgets::{
    compact_choice_button, engine_toggle, float_parameter_row, format_msaa, mark_custom_if_changed,
    section_card, setting_group_label, setting_label,
};

impl PreStartGraphicsApp {
    pub(super) fn show_anti_aliasing(&mut self, ui: &mut egui::Ui) {
        section_card(
            ui,
            "Anti-Aliasing Stack",
            "MSAA, FXAA and TAA remain independent core variables",
            |ui| {
                setting_group_label(ui, "MULTISAMPLE");
                ui.horizontal_wrapped(|ui| {
                    for samples in [0_u8, 2, 4, 8] {
                        let selected = self.settings.graphics.msaa_samples == samples;
                        if compact_choice_button(ui, &format_msaa(samples), selected).clicked() {
                            self.settings.graphics.msaa_samples = samples;
                            self.settings.graphics.mark_custom();
                        }
                    }
                });
                ui.add_space(10.0);
                egui::Grid::new("newengine_prestart_aa_stack")
                    .num_columns(2)
                    .spacing([28.0, 11.0])
                    .show(ui, |ui| {
                        setting_label(ui, "FXAA", "Post-process edge filtering");
                        let changed =
                            engine_toggle(ui, &mut self.settings.graphics.fxaa_enabled, "Enabled");
                        mark_custom_if_changed(&mut self.settings, changed);
                        ui.end_row();
                        float_parameter_row(
                            ui,
                            "FXAA edge threshold",
                            self.settings.graphics.fxaa_enabled,
                            &mut self.settings.graphics.fxaa_edge_threshold,
                            0.01..=1.0,
                            0.005,
                        );
                        float_parameter_row(
                            ui,
                            "FXAA minimum threshold",
                            self.settings.graphics.fxaa_enabled,
                            &mut self.settings.graphics.fxaa_edge_threshold_min,
                            0.001..=1.0,
                            0.001,
                        );
                        float_parameter_row(
                            ui,
                            "FXAA subpixel quality",
                            self.settings.graphics.fxaa_enabled,
                            &mut self.settings.graphics.fxaa_subpixel_quality,
                            0.0..=1.0,
                            0.01,
                        );

                        setting_label(ui, "TAA", "Temporal history resolve and jitter");
                        let changed =
                            engine_toggle(ui, &mut self.settings.graphics.taa_enabled, "Enabled");
                        mark_custom_if_changed(&mut self.settings, changed);
                        ui.end_row();
                        float_parameter_row(
                            ui,
                            "TAA feedback",
                            self.settings.graphics.taa_enabled,
                            &mut self.settings.graphics.taa_feedback,
                            0.0..=0.99,
                            0.01,
                        );
                        float_parameter_row(
                            ui,
                            "TAA neighborhood clamp",
                            self.settings.graphics.taa_enabled,
                            &mut self.settings.graphics.taa_neighborhood_clamping,
                            0.0..=4.0,
                            0.02,
                        );
                        float_parameter_row(
                            ui,
                            "TAA jitter scale",
                            self.settings.graphics.taa_enabled,
                            &mut self.settings.graphics.taa_jitter_scale,
                            0.0..=2.0,
                            0.01,
                        );
                    });
            },
        );
    }
}
