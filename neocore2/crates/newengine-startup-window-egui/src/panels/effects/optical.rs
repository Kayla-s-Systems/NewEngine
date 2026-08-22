#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;

use super::super::super::app::PreStartGraphicsApp;
use super::super::super::widgets::{
    engine_toggle, float_parameter_row, mark_custom_if_changed, section_card, setting_label,
};

impl PreStartGraphicsApp {
    pub(super) fn show_optical_stack(&mut self, ui: &mut egui::Ui) {
        section_card(
            ui,
            "Optical Stack",
            "Highlight bloom and solar lens effects",
            |ui| {
                egui::Grid::new("newengine_prestart_optical_stack")
                    .num_columns(2)
                    .spacing([28.0, 11.0])
                    .show(ui, |ui| {
                        setting_label(
                            ui,
                            "Bloom",
                            "HDR highlight extraction and mip-chain composite",
                        );
                        let changed =
                            engine_toggle(ui, &mut self.settings.graphics.bloom_enabled, "Enabled");
                        mark_custom_if_changed(&mut self.settings, changed);
                        ui.end_row();
                        float_parameter_row(
                            ui,
                            "Bloom threshold",
                            self.settings.graphics.bloom_enabled,
                            &mut self.settings.graphics.bloom_threshold,
                            0.0..=20.0,
                            0.02,
                        );
                        float_parameter_row(
                            ui,
                            "Bloom knee",
                            self.settings.graphics.bloom_enabled,
                            &mut self.settings.graphics.bloom_knee,
                            0.0..=5.0,
                            0.01,
                        );
                        float_parameter_row(
                            ui,
                            "Bloom intensity",
                            self.settings.graphics.bloom_enabled,
                            &mut self.settings.graphics.bloom_intensity,
                            0.0..=5.0,
                            0.005,
                        );
                        float_parameter_row(
                            ui,
                            "Bloom radius",
                            self.settings.graphics.bloom_enabled,
                            &mut self.settings.graphics.bloom_radius,
                            0.1..=5.0,
                            0.01,
                        );

                        setting_label(
                            ui,
                            "Sun rays / lens effects",
                            "Enables solar glare and radial ray contribution",
                        );
                        let changed = engine_toggle(
                            ui,
                            &mut self.settings.graphics.sun_rays_enabled,
                            "Enabled",
                        );
                        mark_custom_if_changed(&mut self.settings, changed);
                        ui.end_row();
                    });
            },
        );
    }
}
