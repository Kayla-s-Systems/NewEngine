#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_gizmo::GizmoMode;
use newengine_platform_winit::egui;

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::SidePanel::left("toolbar")
        .resizable(false)
        .exact_width(56.0)
        .show(ctx, |ui| {
            ui.add_space(6.0);

            ui.vertical_centered(|ui| {
                ui.label("Tools");
            });
            ui.separator();
            ui.add_space(4.0);

            let button = |ui: &mut egui::Ui, label: &str, active: bool| -> egui::Response {
                let mut b = egui::Button::new(label).min_size(egui::vec2(44.0, 36.0));
                if active {
                    b = b.fill(ui.visuals().selection.bg_fill);
                }
                ui.add(b)
            };

            ui.vertical(|ui| {
                if button(ui, "Q", me.editor.active_tool == newengine_editor_core::ToolId::Select)
                    .on_hover_text("Select (Q)")
                    .clicked()
                {
                    me.editor.active_tool = newengine_editor_core::ToolId::Select;
                }

                if button(ui, "W", me.gizmo.mode() == GizmoMode::Translate)
                    .on_hover_text("Move (W)")
                    .clicked()
                {
                    me.gizmo.set_mode(GizmoMode::Translate);
                    me.editor.active_tool = newengine_editor_core::ToolId::Translate;
                }

                if button(ui, "E", me.gizmo.mode() == GizmoMode::Rotate)
                    .on_hover_text("Rotate (E)")
                    .clicked()
                {
                    me.gizmo.set_mode(GizmoMode::Rotate);
                    me.editor.active_tool = newengine_editor_core::ToolId::Rotate;
                }

                if button(ui, "R", me.gizmo.mode() == GizmoMode::Scale)
                    .on_hover_text("Scale (R)")
                    .clicked()
                {
                    me.gizmo.set_mode(GizmoMode::Scale);
                    me.editor.active_tool = newengine_editor_core::ToolId::Scale;
                }
            });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.separator();
                ui.add_space(4.0);
                ui.label("NewEngine");
            });
        });
}
