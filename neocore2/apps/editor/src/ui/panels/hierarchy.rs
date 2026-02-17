#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_platform_winit::egui;
use newengine_primitives::Primitive;
use newengine_scene::components::Name;

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::SidePanel::left("hierarchy")
        .resizable(true)
        .default_width(240.0)
        .min_width(200.0)
        .show(ctx, |ui| {
            ui.heading("Hierarchy");
            ui.add_space(6.0);

            let primary = me.editor.selection.primary();
            let mods = ctx.input(|i| i.modifiers);

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

            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                for (name, e, has_prim) in items {
                    let mut label = name;
                    if has_prim {
                        label.push_str("  [Prim]");
                    }

                    let is_sel = me.editor.selection.contains(e);
                    let is_primary = primary == Some(e);
                    let sel = ui.selectable_label(is_sel, label);

                    if sel.clicked() {
                        if mods.command {
                            me.editor.selection.toggle(e);
                        } else if mods.shift {
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
