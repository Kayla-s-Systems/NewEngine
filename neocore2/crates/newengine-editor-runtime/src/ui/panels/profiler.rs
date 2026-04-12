#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::super::widgets;
use super::super::EditorUiBuild;

pub(crate) fn draw_content(me: &mut EditorUiBuild, ui: &mut egui::Ui) {
    let surface = me.surface_context();
    widgets::panel_title(ui, "Profiler", "Editor shell counters and frame-facing diagnostics");

    widgets::section_card(ui, "Frame Surface", |ui| {
        widgets::stat_row(ui, "Play Mode", format!("{:?}", surface.play_mode));
        widgets::stat_row(ui, "Viewport Mode", surface.viewport_mode.label());
        widgets::stat_row(ui, "Active Tool", format!("{:?}", surface.active_tool));
        widgets::stat_row(ui, "Camera Speed", surface.camera_speed_label);
        widgets::stat_row(ui, "Collision Overlay", if surface.collision_overlay { "On" } else { "Off" });
    });

    ui.add_space(6.0);
    widgets::section_card(ui, "Scene Counters", |ui| {
        widgets::stat_row(ui, "Entities", surface.entity_count.to_string());
        widgets::stat_row(ui, "Selection", surface.selection_count.to_string());
        widgets::stat_row(
            ui,
            "Viewport Extent",
            me.last_viewport_extent
                .map(|(w, h)| format!("{} x {}", w, h))
                .unwrap_or_else(|| "Not initialized".to_string()),
        );
        widgets::stat_row(ui, "Undo Depth", me.editor.commands.len_undo().to_string());
        widgets::stat_row(ui, "Redo Depth", me.editor.commands.len_redo().to_string());
    });

    ui.add_space(6.0);
    widgets::section_card(ui, "Input Snapshot", |ui| {
        widgets::stat_row(ui, "Mouse Δ", format!("{:.2}, {:.2}", me.frame_input.mouse_delta.0, me.frame_input.mouse_delta.1));
        widgets::stat_row(ui, "Wheel", format!("{:.2}, {:.2}", me.frame_input.mouse_wheel.0, me.frame_input.mouse_wheel.1));
        widgets::stat_row(ui, "Fly Capture", if me.fly_latch.is_captured() { "Captured" } else { "Released" });
    });
}
