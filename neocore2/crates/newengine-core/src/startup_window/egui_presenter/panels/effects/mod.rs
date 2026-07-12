#![forbid(unsafe_op_in_unsafe_fn)]

mod ambient_depth;
mod anti_aliasing;
mod optical;

use eframe::egui;

use super::super::app::PreStartGraphicsApp;
use super::super::widgets::warning_banner;

impl PreStartGraphicsApp {
    pub(in crate::startup_window::egui_presenter) fn show_effects(&mut self, ui: &mut egui::Ui) {
        if let Some(message) = self.aa_stack_warning() {
            warning_banner(ui, message);
            ui.add_space(12.0);
        }

        self.show_anti_aliasing(ui);
        ui.add_space(12.0);
        self.show_ambient_depth(ui);
        ui.add_space(12.0);
        self.show_optical_stack(ui);
    }

    fn aa_stack_warning(&self) -> Option<&'static str> {
        let graphics = &self.settings.graphics;
        if graphics.msaa_samples >= 4 && graphics.taa_enabled && graphics.fxaa_enabled {
            Some("MSAA + TAA + FXAA are all enabled. This is valid but usually redundant and expensive.")
        } else if graphics.msaa_samples > 0 && graphics.taa_enabled {
            Some("MSAA and TAA are combined. Verify the intended resolve order in the active renderer backend.")
        } else if graphics.msaa_samples > 0 && graphics.fxaa_enabled {
            Some("MSAA and FXAA are combined. FXAA may soften an already multisampled image.")
        } else if graphics.ssao_enabled
            && !graphics.ssao_half_resolution
            && graphics.ssao_quality_steps >= 32
        {
            Some("Full-resolution SSAO with 32+ steps is a high-bandwidth configuration.")
        } else {
            None
        }
    }
}
