#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("ne_status_bar")
        .resizable(false)
        .exact_height(22.0)
        .show(ctx, |ui| {
            let surface = me.surface_context();
            ui.horizontal(|ui| {
                ui.label(format!("Mode: {:?}", surface.play_mode));
                ui.separator();
                ui.label(format!("Selection: {}", surface.selection_count));
                ui.separator();
                ui.label(format!("Entities: {}", surface.entity_count));
                ui.separator();
                ui.label(format!("Cam: {}", surface.camera_speed_label));
                if surface.collision_overlay {
                    ui.separator();
                    ui.label("Show: Collision");
                }
            });
        });
}
