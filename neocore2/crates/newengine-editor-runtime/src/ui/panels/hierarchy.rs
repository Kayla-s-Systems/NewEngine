#![forbid(unsafe_op_in_unsafe_fn)]

use egui;
use newengine_lighting::{DirectionalLight, PointLight};
use newengine_primitives::Primitive;
use newengine_scene::components::Name;
use newengine_ui::BuiltinUiIcon;

use super::super::util;
use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    let max_w = util::outliner_max_width(ctx, me.layout.show_left_toolbar, me.layout.show_details);

    egui::SidePanel::left("hierarchy")
        .resizable(true)
        .default_width(240.0)
        .min_width(200.0)
        .max_width(max_w)
        .show(ctx, |ui| {
            ui.heading("World Outliner");

            ui.horizontal(|ui| {
                ui.label("Search");
                ui.add(
                    egui::TextEdit::singleline(&mut me.outliner_filter)
                        .hint_text("filter...")
                        .desired_width(f32::INFINITY),
                );
                if me
                    .icons
                    .icon_button(ui, BuiltinUiIcon::Close, "")
                    .on_hover_text("Clear")
                    .clicked()
                {
                    me.outliner_filter.clear();
                }
            });

            ui.add_space(6.0);

            let primary = me.editor.selection.primary();

            let scene = me.scene_bridge.scene();
            let world = scene.read();
            let w = world.world();

            let mut items: Vec<(String, newengine_ecs::EntityId, bool)> = Vec::new();
            for (e, name) in w.query::<Name>() {
                let has_prim = w.get::<Primitive>(e).is_some();
                items.push((name.as_str().to_string(), e, has_prim));
            }
            items.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.stable_u64().cmp(&b.1.stable_u64()))
            });

            let filter = me.outliner_filter.trim().to_ascii_lowercase();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for (name, e, has_prim) in items {
                        if !filter.is_empty() && !name.to_ascii_lowercase().contains(&filter) {
                            continue;
                        }

                        let icon_kind = if w.get::<DirectionalLight>(e).is_some() {
                            Some(BuiltinUiIcon::LightDirectional)
                        } else if w.get::<PointLight>(e).is_some() {
                            Some(BuiltinUiIcon::LightPoint)
                        } else {
                            None
                        };

                        let mut label = name;
                        if has_prim {
                            label.push_str("  [Prim]");
                        }

                        let is_sel = me.editor.selection.contains(e);
                        let is_primary = primary == Some(e);

                        let sel = ui
                            .horizontal(|ui| {
                                if let Some(kind) = icon_kind {
                                    if let Some(tid) = me.icons.tex_id(kind) {
                                        let st = egui::load::SizedTexture::new(
                                            tid,
                                            egui::vec2(16.0, 16.0),
                                        );
                                        ui.image(st);
                                    } else {
                                        ui.add_space(16.0);
                                    }
                                } else {
                                    ui.add_space(16.0);
                                }

                                ui.selectable_label(is_sel, label)
                            })
                            .inner;

                        if sel.clicked() {
                            if me.command_down() {
                                me.editor.selection.toggle(e);
                            } else if me.shift_down() {
                                me.editor.selection.add(e);
                            } else {
                                me.editor.selection.set_single(Some(e));
                            }
                            me.scene_bridge.set_selection(me.editor.selection.primary());
                            if is_primary {
                                // Keep inspector cache stable.
                                if let Some(p) = me.editor.selection.primary() {
                                    me.refresh_inspector_cache(p);
                                }
                            }
                        }
                    }
                });

            ui.separator();
            if ui.button("Deselect").clicked() {
                me.editor.selection.clear();
                me.scene_bridge.set_selection(None);
            }
        });
}
