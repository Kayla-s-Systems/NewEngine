#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::super::{providers, theme, EditorUiBuild};

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    if !me.command_palette.open {
        return;
    }

    let mut open = me.command_palette.open;
    let mut request_close = false;
    let mut run_index = None;
    let query = me.command_palette.query.trim().to_ascii_lowercase();
    let actions = providers::command_palette_actions(me);
    let mut matches = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        if query.is_empty()
            || action.label.to_ascii_lowercase().contains(&query)
            || action.keywords.to_ascii_lowercase().contains(&query)
        {
            matches.push((index, action));
        }
    }
    if me.command_palette.selected_index >= matches.len() {
        me.command_palette.selected_index = 0;
    }

    egui::Window::new("Command Palette")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
        .show(ctx, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut me.command_palette.query)
                    .hint_text("Search actions, modes, panels, tools... (Ctrl+P)")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);

            let enter_pressed = me.key_pressed(newengine_ui::input::keys::ENTER);
            let esc_pressed = me.key_pressed(newengine_ui::input::keys::ESCAPE);

            if esc_pressed {
                request_close = true;
            }

            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for (visible_index, (action_index, action)) in matches.iter().enumerate() {
                    let selected = visible_index == me.command_palette.selected_index;
                    let response = theme::selectable_row(
                        ui,
                        selected,
                        action.label.as_ref(),
                        action.keywords.as_ref(),
                    );
                    if response.clicked() {
                        run_index = Some(*action_index);
                    }
                }
            });

            if enter_pressed && !matches.is_empty() {
                run_index = Some(matches[me.command_palette.selected_index].0);
            }
        });

    if let Some(index) = run_index {
        if let Some(action) = actions.get(index) {
            me.execute_ui_action(&action.action);
        }
        me.command_palette.query.clear();
        me.command_palette.selected_index = 0;
        request_close = true;
    }

    if request_close {
        open = false;
    }
    me.command_palette.open = open;
}
