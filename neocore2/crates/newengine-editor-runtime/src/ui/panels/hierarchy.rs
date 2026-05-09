#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;

use egui;
use newengine_ecs::EntityId;
use newengine_lighting::{DirectionalLight, PointLight};
use newengine_primitives::Primitive;
use newengine_scene::components::Name;
use newengine_transform::Parent;
use newengine_ui::BuiltinUiIcon;

use crate::gameplay::{DisplayMode, DisplayVisibility, PlayerActor};

use super::super::schema;
use super::super::theme;
use super::super::widgets;
use super::super::EditorUiBuild;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HierarchyLayerKind {
    Actors,
    Lights,
    Buildings,
    Units,
    Foliage,
    Debug,
}

#[derive(Debug, Clone)]
struct HierarchyItem {
    id: EntityId,
    name: String,
    parent: Option<EntityId>,
    has_primitive: bool,
    is_directional_light: bool,
    is_point_light: bool,
    is_player: bool,
    display_mode: DisplayMode,
}

impl HierarchyItem {
    #[inline]
    fn kind_tag(&self) -> &'static str {
        if self.is_directional_light {
            "Directional Light"
        } else if self.is_point_light {
            "Point Light"
        } else if self.is_player {
            "Player"
        } else if self.has_primitive {
            "Primitive"
        } else {
            "Actor"
        }
    }

    #[inline]
    fn display_tag(&self) -> &'static str {
        match self.display_mode {
            DisplayMode::Both => "",
            DisplayMode::EditorOnly => "EditorOnly",
            DisplayMode::GameOnly => "GameOnly",
        }
    }

    #[inline]
    fn layer(&self) -> HierarchyLayerKind {
        if self.is_directional_light || self.is_point_light {
            return HierarchyLayerKind::Lights;
        }
        if self.is_player {
            return HierarchyLayerKind::Units;
        }

        let lower = self.name.to_ascii_lowercase();
        if ["unit", "npc", "enemy", "pawn", "character"]
            .iter()
            .any(|needle| lower.contains(needle))
        {
            HierarchyLayerKind::Units
        } else if ["building", "house", "wall", "tower", "gate", "fort"]
            .iter()
            .any(|needle| lower.contains(needle))
        {
            HierarchyLayerKind::Buildings
        } else if ["foliage", "tree", "grass", "bush", "plant"]
            .iter()
            .any(|needle| lower.contains(needle))
        {
            HierarchyLayerKind::Foliage
        } else if ["debug", "helper", "gizmo", "probe"]
            .iter()
            .any(|needle| lower.contains(needle))
        {
            HierarchyLayerKind::Debug
        } else {
            HierarchyLayerKind::Actors
        }
    }

    #[inline]
    fn matches_text(&self, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }

        let display = self.display_tag();
        self.name.to_ascii_lowercase().contains(filter)
            || self.kind_tag().to_ascii_lowercase().contains(filter)
            || (!display.is_empty() && display.to_ascii_lowercase().contains(filter))
            || self.id.stable_u64().to_string().contains(filter)
    }
}

#[inline]
fn is_layer_enabled(me: &EditorUiBuild, layer: HierarchyLayerKind) -> bool {
    match layer {
        HierarchyLayerKind::Actors => me.scene_layers.actors,
        HierarchyLayerKind::Lights => me.scene_layers.lights,
        HierarchyLayerKind::Buildings => me.scene_layers.buildings,
        HierarchyLayerKind::Units => me.scene_layers.units,
        HierarchyLayerKind::Foliage => me.scene_layers.foliage,
        HierarchyLayerKind::Debug => me.scene_layers.debug,
    }
}

fn node_visible_recursive(
    items: &HashMap<EntityId, HierarchyItem>,
    children: &HashMap<EntityId, Vec<EntityId>>,
    id: EntityId,
    filter: &str,
    me: &EditorUiBuild,
) -> bool {
    let Some(item) = items.get(&id) else {
        return false;
    };

    let self_visible = is_layer_enabled(me, item.layer()) && item.matches_text(filter);
    if self_visible {
        return true;
    }

    children
        .get(&id)
        .map(|entries| {
            entries
                .iter()
                .copied()
                .any(|child| node_visible_recursive(items, children, child, filter, me))
        })
        .unwrap_or(false)
}

fn draw_tree_node(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    items: &HashMap<EntityId, HierarchyItem>,
    children: &HashMap<EntityId, Vec<EntityId>>,
    id: EntityId,
    depth: usize,
    filter: &str,
) {
    if !node_visible_recursive(items, children, id, filter, me) {
        return;
    }

    let Some(item) = items.get(&id) else {
        return;
    };

    let is_sel = me.editor.selection.contains(id);
    let subtitle = if item.display_tag().is_empty() {
        format!("{} · #{}", item.kind_tag(), item.id.stable_u64())
    } else {
        format!(
            "{} · {} · #{}",
            item.kind_tag(),
            item.display_tag(),
            item.id.stable_u64()
        )
    };

    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 14.0);

        let has_children = children.get(&id).map(|v| !v.is_empty()).unwrap_or(false);
        if has_children {
            ui.label(egui::RichText::new("▾").small().weak());
        } else {
            ui.add_space(10.0);
        }

        let icon_kind = if item.is_directional_light {
            Some(BuiltinUiIcon::LightDirectional)
        } else if item.is_point_light {
            Some(BuiltinUiIcon::LightPoint)
        } else {
            None
        };

        if let Some(kind) = icon_kind {
            if let Some(tid) = me.icons.tex_id(kind) {
                let st = egui::load::SizedTexture::new(tid, egui::vec2(14.0, 14.0));
                ui.image(st);
            } else {
                ui.add_space(16.0);
            }
        } else {
            ui.add_space(16.0);
        }

        let response = ui.add_sized(
            [ui.available_width(), 30.0],
            egui::Button::selectable(is_sel, format!("{}\n{}", item.name, subtitle)),
        );

        if response.clicked() {
            if me.command_down() {
                me.editor.selection.toggle(id);
            } else if me.shift_down() {
                me.editor.selection.add(id);
            } else {
                me.editor.selection.set_single(Some(id));
            }
            me.scene_bridge.set_selection(me.editor.selection.primary());
            if let Some(primary) = me.editor.selection.primary() {
                me.refresh_inspector_cache(primary);
            }
        }

        if response.drag_started() {
            me.hierarchy_drag_source = Some(id);
        }

        if let Some(source) = me.hierarchy_drag_source {
            if source != id && response.hovered() && ui.input(|i| i.pointer.any_released()) {
                me.scene_bridge.cmd_set_parent(source, Some(id));
                me.hierarchy_drag_source = None;
            }
        }

        response.context_menu(|ui| {
            let primary = me.editor.selection.primary();
            let selection_ctx = schema::build_selection_context(me, id);
            for action in schema::selection_context_actions(me, Some(&selection_ctx)) {
                if ui
                    .add_enabled(
                        action.enabled,
                        egui::Button::selectable(action.selected, action.label),
                    )
                    .clicked()
                {
                    if primary != Some(id) {
                        me.editor.selection.set_single(Some(id));
                        me.scene_bridge.set_selection(Some(id));
                        me.refresh_inspector_cache(id);
                    }
                    me.execute_context_action(action.id);
                    ui.close();
                }
            }
        });
    });

    if let Some(child_ids) = children.get(&id) {
        for child in child_ids {
            draw_tree_node(me, ui, items, children, *child, depth + 1, filter);
        }
    }
}

pub(crate) fn draw_content(me: &mut EditorUiBuild, ui: &mut egui::Ui) {
    let (items, children, roots) = {
        let scene = me.scene_bridge.scene();
        let guard = scene.read();
        let world = guard.world();

        let mut items = HashMap::<EntityId, HierarchyItem>::new();
        for (id, name) in world.query::<Name>() {
            let item = HierarchyItem {
                id,
                name: name.as_str().to_string(),
                parent: world.get::<Parent>(id).map(|parent| parent.0),
                has_primitive: world.get::<Primitive>(id).is_some(),
                is_directional_light: world.get::<DirectionalLight>(id).is_some(),
                is_point_light: world.get::<PointLight>(id).is_some(),
                is_player: world.get::<PlayerActor>(id).is_some(),
                display_mode: world
                    .get::<DisplayVisibility>(id)
                    .copied()
                    .unwrap_or_default()
                    .mode,
            };
            items.insert(id, item);
        }

        let mut children = HashMap::<EntityId, Vec<EntityId>>::new();
        let mut roots = Vec::new();
        for (id, item) in &items {
            if let Some(parent) = item.parent.filter(|parent| items.contains_key(&*parent)) {
                children.entry(parent).or_default().push(*id);
            } else {
                roots.push(*id);
            }
        }

        let sort_ids = |ids: &mut Vec<EntityId>| {
            ids.sort_by(|a, b| {
                let an = items.get(&*a).map(|item| item.name.as_str()).unwrap_or("");
                let bn = items.get(&*b).map(|item| item.name.as_str()).unwrap_or("");
                an.cmp(bn)
                    .then_with(|| a.stable_u64().cmp(&b.stable_u64()))
            });
        };

        sort_ids(&mut roots);
        for ids in children.values_mut() {
            sort_ids(ids);
        }

        (items, children, roots)
    };

    let filter = me.outliner_filter.trim().to_ascii_lowercase();
    let total_count = items.len();
    let visible_count = roots
        .iter()
        .copied()
        .filter(|id| node_visible_recursive(&items, &children, *id, &filter, me))
        .count();

    widgets::panel_title(
        ui,
        "Scene Hierarchy",
        &format!("{} roots · {} entities", visible_count, total_count),
    );
    widgets::search_field(ui, &mut me.outliner_filter, "Search entities, ids, display mode...");

    ui.horizontal_wrapped(|ui| {
        widgets::filter_chip(ui, &mut me.scene_layers.actors, "Actors");
        widgets::filter_chip(ui, &mut me.scene_layers.lights, "Lights");
        widgets::filter_chip(ui, &mut me.scene_layers.buildings, "Buildings");
        widgets::filter_chip(ui, &mut me.scene_layers.units, "Units");
        widgets::filter_chip(ui, &mut me.scene_layers.foliage, "Foliage");
        widgets::filter_chip(ui, &mut me.scene_layers.debug, "Debug");
        if ui.small_button("Reset").clicked() {
            me.outliner_filter.clear();
            me.scene_layers = Default::default();
        }
    });

    if let Some(source) = me.hierarchy_drag_source {
        theme::section_frame(ui).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("Dragging entity #{}", source.stable_u64())).small().strong());
                ui.separator();
                ui.label(egui::RichText::new("Release over another row to parent, or here to attach to scene root.").small().weak());
                if ui.button("Parent to Root").clicked() {
                    me.scene_bridge.cmd_set_parent(source, None);
                    me.hierarchy_drag_source = None;
                }
            });
        });
    }

    ui.add_space(4.0);
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for root in roots {
            draw_tree_node(me, ui, &items, &children, root, 0, &filter);
        }
    });

    if ui.input(|i| i.pointer.any_released()) {
        me.hierarchy_drag_source = None;
    }
}
