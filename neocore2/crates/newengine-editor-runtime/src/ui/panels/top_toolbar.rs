#![forbid(unsafe_op_in_unsafe_fn)]

use egui;
use newengine_editor_core::ToolId;
use newengine_ui::BuiltinUiIcon;

use super::super::{providers, theme, EditorUiBuild};

fn compact_button(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    theme::toolbar_button(ui, selected, text)
}

fn tool_button(ui: &mut egui::Ui, me: &mut EditorUiBuild, desc: &providers::UiActionDescriptor) {
    let response = match &desc.action {
        providers::UiAction::SetTool(tool) => {
            let (icon, fallback) = match tool {
                ToolId::Select => (None, "Q"),
                ToolId::Translate => (me.icons.tex_id(BuiltinUiIcon::GizmoTranslate), "W"),
                ToolId::Rotate => (me.icons.tex_id(BuiltinUiIcon::GizmoRotate), "E"),
                ToolId::Scale => (me.icons.tex_id(BuiltinUiIcon::GizmoScale), "R"),
            };
            if let Some(tid) = icon {
                let st = egui::load::SizedTexture::new(tid, egui::vec2(14.0, 14.0));
                let fill = if desc.selected {
                    ui.visuals().selection.bg_fill
                } else {
                    ui.visuals().widgets.inactive.bg_fill
                };
                let stroke = if desc.selected {
                    ui.visuals().selection.stroke
                } else {
                    ui.visuals().widgets.inactive.bg_stroke
                };
                ui.add(
                    egui::Button::image(st)
                        .fill(fill)
                        .stroke(stroke)
                        .min_size(egui::vec2(24.0, 24.0))
                        .corner_radius(egui::CornerRadius::same(5)),
                )
                    .on_hover_text(desc.label.as_ref())
            } else {
                compact_button(ui, desc.selected, fallback).on_hover_text(desc.label.as_ref())
            }
        }
        _ => compact_button(ui, desc.selected, desc.label.as_ref()),
    };
    if response.clicked() {
        me.execute_ui_action(&desc.action);
    }
}

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::TopBottomPanel::top("ne_toolbar")
        .resizable(false)
        .exact_height(44.0)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if let Some(tid) = me.icons.tex_id(BuiltinUiIcon::AppLogo) {
                    let st = egui::load::SizedTexture::new(tid, egui::vec2(16.0, 16.0));
                    ui.image(st);
                }
                ui.label(egui::RichText::new("NewEngine Editor").strong());
                ui.separator();

                for desc in providers::file_toolbar_actions(me).into_iter().take(3) {
                    let response = compact_button(ui, desc.selected, desc.label.as_ref());
                    if response.clicked() {
                        me.execute_ui_action(&desc.action);
                    }
                }

                ui.separator();
                for desc in providers::tool_actions(me) {
                    tool_button(ui, me, &desc);
                }

                ui.separator();
                ui.menu_button("Add", |ui| {
                    for (group_index, group) in providers::create_menu_groups(me).into_iter().enumerate() {
                        if group_index > 0 {
                            ui.separator();
                        }
                        ui.label(egui::RichText::new(group.label).small().strong());
                        for desc in group.actions {
                            if ui.add_enabled(desc.enabled, egui::Button::new(desc.label.as_ref())).clicked() {
                                me.execute_ui_action(&desc.action);
                                ui.close();
                            }
                        }
                    }
                });

                egui::ComboBox::from_id_salt("workspace_preset_compact")
                    .width(110.0)
                    .selected_text(me.workspace_preset.label())
                    .show_ui(ui, |ui| {
                        for choice in providers::workspace_preset_choices(me) {
                            if ui.add_enabled(choice.enabled, egui::Button::selectable(choice.selected, choice.label)).clicked() {
                                me.execute_ui_action(&providers::UiAction::SetWorkspacePreset(choice.value));
                                ui.close();
                            }
                        }
                    });

                ui.separator();
                for desc in providers::runtime_actions(me) {
                    let response = compact_button(ui, desc.selected, desc.label.as_ref());
                    if response.clicked() {
                        me.execute_ui_action(&desc.action);
                    }
                }

                ui.separator();
                if compact_button(ui, false, "Save Layout").clicked() {
                    me.save_dock_layout_snapshot();
                }
                if compact_button(ui, false, "Load Layout").clicked() {
                    me.restore_dock_layout_snapshot();
                }
                if compact_button(ui, false, "Reset Layout").clicked() {
                    me.reset_dock_layout();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if compact_button(ui, false, "Actions").clicked() {
                        me.open_command_palette();
                    }
                    ui.separator();
                    ui.label(format!("Sel {} · Ent {}", me.editor.selection.len(), me.scene_bridge.scene().read().world().entity_count()));
                });
            });
        });
}
