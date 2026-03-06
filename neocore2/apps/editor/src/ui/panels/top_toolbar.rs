#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_gizmo::GizmoMode;
use newengine_platform_winit::egui;
use newengine_ui::BuiltinUiIcon;

use super::super::EditorUiBuild;

fn icon_only(
    ui: &mut egui::Ui,
    tid: Option<egui::TextureId>,
    fallback: &str,
    tooltip: &str,
) -> egui::Response {
    let min = egui::vec2(28.0, 28.0);
    match tid {
        Some(tid) => {
            let st = egui::load::SizedTexture::new(tid, egui::vec2(16.0, 16.0));
            ui.add(egui::Button::image(st).min_size(min)).on_hover_text(tooltip)
        }
        None => ui
            .add(egui::Button::new(fallback).min_size(min))
            .on_hover_text(tooltip),
    }
}

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::TopBottomPanel::top("ne_toolbar")
        .resizable(false)
        .exact_height(36.0)
        .show(ctx, |ui| {
            ui.add_space(2.0);

            ui.horizontal(|ui| {
                // File operations.
                let has_scene_io = me.scene_io.is_some();

                if icon_only(
                    ui,
                    me.icons.tex_id(BuiltinUiIcon::FileNew),
                    "New",
                    "New Scene (Ctrl+N)",
                )
                    .clicked()
                {
                    me.scene_bridge.cmd_new_scene();
                    me.editor.commands.clear();
                    me.editor.selection.clear();
                }

                let open_resp = ui
                    .add_enabled_ui(has_scene_io, |ui| {
                        icon_only(
                            ui,
                            me.icons.tex_id(BuiltinUiIcon::FileOpen),
                            "Open",
                            "Load Scene (Ctrl+O)",
                        )
                    })
                    .inner;
                if open_resp.clicked() {
                    me.scene_io_ui.open = true;
                    me.scene_io_ui.mode = super::super::SceneIoMode::Load;
                }

                let save_resp = ui
                    .add_enabled_ui(has_scene_io, |ui| {
                        icon_only(
                            ui,
                            me.icons.tex_id(BuiltinUiIcon::FileSave),
                            "Save",
                            "Save Scene (Ctrl+S)",
                        )
                    })
                    .inner;
                if save_resp.clicked() {
                    me.scene_io_ui.open = true;
                    me.scene_io_ui.mode = super::super::SceneIoMode::Save;
                }

                ui.separator();

                // AssetManager.
                let has_assets = me.assets.is_some();
                let asset_tid = me.icons.tex_id(BuiltinUiIcon::AssetManager);
                let asset_resp = ui.add_enabled_ui(has_assets, |ui| {
                    icon_only(ui, asset_tid, "Assets", "Asset Manager")
                });
                if asset_resp.inner.clicked() {
                    me.asset_ui.open = true;
                }

                ui.separator();

                // Gizmo/tools.
                let active_tool = me.editor.active_tool;

                let select_active = active_tool == newengine_editor_core::ToolId::Select;
                let fill_select = if select_active {
                    ui.visuals().selection.bg_fill
                } else {
                    egui::Color32::TRANSPARENT
                };

                if ui
                    .add(
                        egui::Button::new("Q")
                            .min_size(egui::vec2(28.0, 28.0))
                            .fill(fill_select),
                    )
                    .on_hover_text("Select (Q)")
                    .clicked()
                {
                    me.editor.active_tool = newengine_editor_core::ToolId::Select;
                }

                let gizmo_btn = |ui: &mut egui::Ui,
                                 me: &mut EditorUiBuild,
                                 icon: BuiltinUiIcon,
                                 mode: GizmoMode,
                                 tool: newengine_editor_core::ToolId,
                                 hotkey: &'static str| {
                    let active = me.gizmo.mode() == mode;
                    let fill = if active {
                        ui.visuals().selection.bg_fill
                    } else {
                        egui::Color32::TRANSPARENT
                    };

                    let resp = if let Some(tid) = me.icons.tex_id(icon) {
                        let st = egui::load::SizedTexture::new(tid, egui::vec2(16.0, 16.0));
                        ui.add(egui::Button::image(st).min_size(egui::vec2(28.0, 28.0)).fill(fill))
                    } else {
                        ui.add(
                            egui::Button::new(hotkey)
                                .min_size(egui::vec2(28.0, 28.0))
                                .fill(fill),
                        )
                    }
                        .on_hover_text(hotkey);

                    if resp.clicked() {
                        me.gizmo.set_mode(mode);
                        me.editor.active_tool = tool;
                        me.editor.gizmo_mode = match mode {
                            GizmoMode::Translate => newengine_editor_core::GizmoMode::Translate,
                            GizmoMode::Rotate => newengine_editor_core::GizmoMode::Rotate,
                            GizmoMode::Scale => newengine_editor_core::GizmoMode::Scale,
                        };
                    }
                };

                gizmo_btn(
                    ui,
                    me,
                    BuiltinUiIcon::GizmoTranslate,
                    GizmoMode::Translate,
                    newengine_editor_core::ToolId::Translate,
                    "Move (W)",
                );
                gizmo_btn(
                    ui,
                    me,
                    BuiltinUiIcon::GizmoRotate,
                    GizmoMode::Rotate,
                    newengine_editor_core::ToolId::Rotate,
                    "Rotate (E)",
                );
                gizmo_btn(
                    ui,
                    me,
                    BuiltinUiIcon::GizmoScale,
                    GizmoMode::Scale,
                    newengine_editor_core::ToolId::Scale,
                    "Scale (R)",
                );

                ui.separator();

                // Add primitives (registry-driven).
                {
                    let prims = me.scene_bridge.primitives_snapshot();

                    if me.selected_primitive.is_none() {
                        me.selected_primitive = prims.first().map(|p| p.1);
                    }

                    if let Some(sel) = me.selected_primitive {
                        if !prims.iter().any(|p| p.1 == sel) {
                            me.selected_primitive = prims.first().map(|p| p.1);
                        }
                    }

                    let current_label = me
                        .selected_primitive
                        .and_then(|id| prims.iter().find(|x| x.1 == id).map(|x| x.0.as_str()))
                        .unwrap_or("<none>");

                    egui::ComboBox::from_id_salt("ne_add_primitive_combo")
                        .width(180.0)
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            for (name, id) in &prims {
                                let tex = me
                                    .previews
                                    .lock()
                                    .request(*id, newengine_previews::PrimitivePreviewSize::S48);

                                ui.horizontal(|ui| {
                                    if tex.0 != 0 {
                                        let tid = egui::TextureId::User(tex.0 as u64);
                                        let st = egui::load::SizedTexture::new(
                                            tid,
                                            egui::vec2(24.0, 24.0),
                                        );
                                        ui.image(st);
                                    } else {
                                        ui.add_space(24.0);
                                    }
                                    ui.selectable_value(&mut me.selected_primitive, Some(*id), name);
                                });
                            }
                        });

                    if me
                        .icons
                        .icon_button(ui, BuiltinUiIcon::Load, "Add")
                        .clicked()
                    {
                        if let Some(id) = me.selected_primitive {
                            let name = prims
                                .iter()
                                .find(|x| x.1 == id)
                                .map(|x| x.0.clone())
                                .unwrap_or_else(|| "Primitive".to_string());

                            let (cam_pos, cam_fwd) = me.viewport_bridge.read_camera_spawn();
                            let mut p = cam_pos + cam_fwd * 3.0;
                            p.y = p.y.max(0.5);

                            me.scene_bridge.cmd_spawn_primitive(id, name, p);
                        }
                    }
                }

                ui.separator();

                // Lights.
                {
                    use crate::ui::LightSpawnKind;

                    let current_label = match me.selected_light_kind {
                        LightSpawnKind::Directional => "Directional",
                        LightSpawnKind::Point => "Point",
                    };

                    egui::ComboBox::from_id_salt("ne_add_light_combo")
                        .width(140.0)
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut me.selected_light_kind,
                                LightSpawnKind::Directional,
                                "Directional",
                            );
                            ui.selectable_value(
                                &mut me.selected_light_kind,
                                LightSpawnKind::Point,
                                "Point",
                            );
                        });

                    if me
                        .icons
                        .icon_button(
                            ui,
                            match me.selected_light_kind {
                                LightSpawnKind::Directional => BuiltinUiIcon::LightDirectional,
                                LightSpawnKind::Point => BuiltinUiIcon::LightPoint,
                            },
                            "Add",
                        )
                        .clicked()
                    {
                        match me.selected_light_kind {
                            LightSpawnKind::Directional => {
                                me.scene_bridge.cmd_spawn_directional_light(
                                    "Sun".to_string(),
                                    newengine_math::Vec3::new(0.0, 6.0, 0.0),
                                    newengine_math::Vec3::new(-0.35, -1.0, -0.25),
                                );
                            }
                            LightSpawnKind::Point => {
                                me.scene_bridge.cmd_spawn_point_light(
                                    "PointLight".to_string(),
                                    newengine_math::Vec3::new(0.0, 2.0, 0.0),
                                );
                            }
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Ok(mut pm) = me.plugin_manager.lock() {
                        pm.topbar_button(ui);
                    }

                    if me
                        .icons
                        .icon_button(ui, BuiltinUiIcon::Console, "Console")
                        .clicked()
                    {
                        me.console_open = !me.console_open;
                    }
                });
            });
        });
}
