#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;

use newengine_core::startup_window::STARTUP_SETTINGS_SCHEMA_VERSION;

use super::super::app::PreStartGraphicsApp;
use super::super::widgets::{aa_summary, bool_string, diagnostic_row, section_card, variable_row};

impl PreStartGraphicsApp {
    pub(crate) fn show_advanced(&mut self, ui: &mut egui::Ui) {
        section_card(
            ui,
            "Resolved Launch Snapshot",
            "The exact typed state newengine-core will publish before Engine creation",
            |ui| {
                egui::Grid::new("newengine_prestart_snapshot")
                    .num_columns(2)
                    .spacing([28.0, 10.0])
                    .show(ui, |ui| {
                        diagnostic_row(
                            ui,
                            "Schema",
                            &format!("startup_settings/v{}", STARTUP_SETTINGS_SCHEMA_VERSION),
                        );
                        diagnostic_row(ui, "Owner", "newengine-core");
                        diagnostic_row(ui, "Persistence", "config.json / atomic replace");
                        diagnostic_row(
                            ui,
                            "Window",
                            &format!(
                                "{}×{} / {}",
                                self.width,
                                self.height,
                                self.settings.display.window_mode.label()
                            ),
                        );
                        diagnostic_row(
                            ui,
                            "Graphics preset",
                            self.settings.graphics.preset.label(),
                        );
                        diagnostic_row(ui, "AA stack", &aa_summary(&self.settings));
                        diagnostic_row(ui, "Render pressure", self.render_pressure().label());
                    });
            },
        );

        ui.add_space(12.0);
        section_card(
            ui,
            "Exported Process Variables",
            "FFI, plugin and backend consumers receive the same confirmed snapshot",
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("newengine_prestart_variables_scroll")
                    .max_height(320.0)
                    .show(ui, |ui| {
                        egui::Grid::new("newengine_prestart_variables")
                            .num_columns(2)
                            .spacing([28.0, 7.0])
                            .striped(true)
                            .show(ui, |ui| {
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_PRESET",
                                    self.settings.graphics.preset.as_str(),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_MSAA_SAMPLES",
                                    &self.settings.graphics.msaa_samples.to_string(),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_FXAA_ENABLED",
                                    bool_string(self.settings.graphics.fxaa_enabled),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_TAA_ENABLED",
                                    bool_string(self.settings.graphics.taa_enabled),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_SSAO_ENABLED",
                                    bool_string(self.settings.graphics.ssao_enabled),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_BLOOM_ENABLED",
                                    bool_string(self.settings.graphics.bloom_enabled),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_DOF_ENABLED",
                                    bool_string(self.settings.graphics.depth_of_field_enabled),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_MOTION_BLUR_ENABLED",
                                    bool_string(self.settings.graphics.motion_blur_enabled),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_SUN_RAYS_ENABLED",
                                    bool_string(self.settings.graphics.sun_rays_enabled),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_SHADOWS_ENABLED",
                                    bool_string(self.settings.graphics.shadows_enabled),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_SHADOW_QUALITY",
                                    self.settings.graphics.shadow_quality.as_str(),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_SHADOW_CASCADE_COUNT",
                                    &self.settings.graphics.shadow_cascade_count.to_string(),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_SHADOW_MAP_RESOLUTION",
                                    &self.settings.graphics.shadow_map_resolution.to_string(),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_LOD_DISTANCE_SCALE",
                                    &format!("{:.2}", self.settings.graphics.lod_distance_scale),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_GRAPHICS_TEXTURE_QUALITY",
                                    self.settings.graphics.texture_quality.as_str(),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_DISPLAY_WINDOW_MODE",
                                    self.settings.display.window_mode.as_str(),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_DISPLAY_VSYNC",
                                    bool_string(self.settings.display.vsync),
                                );
                                variable_row(
                                    ui,
                                    "NEWENGINE_DISPLAY_FRAME_LIMIT",
                                    &self.settings.display.frame_limit.to_string(),
                                );
                            });
                    });
            },
        );
    }
}
