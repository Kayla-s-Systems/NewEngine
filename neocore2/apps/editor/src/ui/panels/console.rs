#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_platform_winit::egui;

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    if !me.console_open {
        return;
    }

    egui::Window::new("Console")
        .open(&mut me.console_open)
        .resizable(true)
        .vscroll(true)
        .show(ctx, |ui| {
            ui.label("Foundation mode: console is intentionally minimal for now.");
            ui.add_space(6.0);

            let resp = ui.add(
                egui::TextEdit::singleline(&mut me.console_input)
                    .hint_text("type a command (no-op)")
                    .desired_width(f32::INFINITY),
            );

            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                me.console_input.clear();
            }
        });
}
