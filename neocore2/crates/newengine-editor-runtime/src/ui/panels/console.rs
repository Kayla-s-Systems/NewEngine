#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::super::widgets;
use super::super::EditorUiBuild;

pub(crate) fn draw_content(me: &mut EditorUiBuild, ui: &mut egui::Ui) {
    widgets::panel_title(ui, "Console", "Runtime output, quick filter and command scratchpad");
    widgets::search_field(ui, &mut me.console_filter, "Filter logs, systems, commands...");
    ui.add(
        egui::TextEdit::singleline(&mut me.console_input)
            .hint_text("Type a command or note")
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);

    widgets::section_card(ui, "Editor Runtime", |ui| {
        widgets::stat_row(ui, "Play Mode", me.play_mode_label());
        widgets::stat_row(ui, "Viewport", me.viewport_mode.label());
        widgets::stat_row(ui, "Tool", me.active_tool_label());
        widgets::stat_row(ui, "Camera Speed", me.camera_speed.active_label());
        widgets::stat_row(ui, "Selection", format!("{} entities", me.editor.selection.len()));
        widgets::stat_row(ui, "Asset Service", if me.assets.is_some() { "Online" } else { "Offline" });
    });

    ui.add_space(6.0);
    widgets::section_card(ui, "Log Stream", |ui| {
        ui.label(egui::RichText::new("Log routing into this dock panel is still a shell-layer stub.").small().weak());
        ui.label(egui::RichText::new("The panel is now in the dock graph, so wiring a real sink is isolated to this tab instead of the old bottom drawer.").small().weak());
        if !me.console_filter.trim().is_empty() {
            ui.add_space(4.0);
            ui.label(format!("Active filter: {}", me.console_filter.trim()));
        }
    });
}
