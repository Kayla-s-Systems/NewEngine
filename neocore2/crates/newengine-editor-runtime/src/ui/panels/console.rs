#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    if !me.console_open {
        return;
    }

    let enter_pressed = me.key_pressed(newengine_ui::input::keys::ENTER);
    let mut console_open = me.console_open;
    let mut console_input = std::mem::take(&mut me.console_input);

    egui::Window::new("Console")
        .open(&mut console_open)
        .resizable(true)
        .vscroll(true)
        .show(ctx, |ui| {
            ui.label("Foundation mode: console is intentionally minimal for now.");
            ui.add_space(6.0);

            let resp = ui.add(
                egui::TextEdit::singleline(&mut console_input)
                    .hint_text("type a command (no-op)")
                    .desired_width(f32::INFINITY),
            );

            if resp.lost_focus() && enter_pressed {
                console_input.clear();
            }
        });

    me.console_open = console_open;
    me.console_input = console_input;
}