#![forbid(unsafe_op_in_unsafe_fn)]

use egui;
use newengine_gizmo::GizmoMode;
use newengine_ui::BuiltinUiIcon;

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

            let icon_tool_button = |me: &mut EditorUiBuild,
                                    ui: &mut egui::Ui,
                                    icon: Option<BuiltinUiIcon>,
                                    fallback_label: &str,
                                    active: bool|
                -> egui::Response {
                let fill = if active {
                    ui.visuals().selection.bg_fill
                } else {
                    egui::Color32::TRANSPARENT
                };

                match icon.and_then(|i| me.icons.tex_id(i)) {
                    Some(tid) => {
                        let st = egui::load::SizedTexture::new(tid, egui::vec2(20.0, 20.0));
                        ui.add(
                            egui::Button::image(st)
                                .min_size(egui::vec2(44.0, 36.0))
                                .fill(fill),
                        )
                    }
                    None => ui.add(
                        egui::Button::new(fallback_label)
                            .min_size(egui::vec2(44.0, 36.0))
                            .fill(fill),
                    ),
                }
            };

            ui.vertical(|ui| {
                if icon_tool_button(
                    me,
                    ui,
                    None,
                    "Q",
                    me.editor.active_tool == newengine_editor_core::ToolId::Select,
                )
                    .on_hover_text("Select (Q)")
                    .clicked()
                {
                    me.editor.active_tool = newengine_editor_core::ToolId::Select;
                }

                if icon_tool_button(
                    me,
                    ui,
                    Some(BuiltinUiIcon::GizmoTranslate),
                    "W",
                    me.gizmo.mode() == GizmoMode::Translate,
                )
                    .on_hover_text("Move (W)")
                    .clicked()
                {
                    me.gizmo.set_mode(GizmoMode::Translate);
                    me.editor.active_tool = newengine_editor_core::ToolId::Translate;
                }

                if icon_tool_button(
                    me,
                    ui,
                    Some(BuiltinUiIcon::GizmoRotate),
                    "E",
                    me.gizmo.mode() == GizmoMode::Rotate,
                )
                    .on_hover_text("Rotate (E)")
                    .clicked()
                {
                    me.gizmo.set_mode(GizmoMode::Rotate);
                    me.editor.active_tool = newengine_editor_core::ToolId::Rotate;
                }

                if icon_tool_button(
                    me,
                    ui,
                    Some(BuiltinUiIcon::GizmoScale),
                    "R",
                    me.gizmo.mode() == GizmoMode::Scale,
                )
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
