use super::*;

impl SceneBridge {
    pub fn apply_editor_selection_actions(&self, frame: &UiEventDispatchFrame) -> bool {
        let mut applied = false;
        for action in &frame.actions {
            if action.trigger != UiNodeEventTrigger::Click {
                continue;
            }
            let Some(entity_key) =
                selection_entity_key_from_action(action.action_id.as_str(), &action.payload)
            else {
                continue;
            };
            if let Some(entity) = self.select_entity_by_stable_key(entity_key) {
                newengine_ulog_api::ulog::info!(
                    "editor selection: selected entity={:?} stable_key={} via action_id='{}' surface='{}' node='{}' route='engine.editor.selection.select_entity'",
                    entity,
                    entity_key,
                    action.action_id,
                    action.surface_id,
                    action.node_id
                );
                applied = true;
            } else {
                newengine_ulog_api::ulog::warn!(
                    "editor selection: action_id='{}' requested missing entity stable_key={} surface='{}' node='{}'",
                    action.action_id,
                    entity_key,
                    action.surface_id,
                    action.node_id
                );
            }
        }
        applied
    }

    pub fn apply_in_game_editor_actions(&self, frame: &UiEventDispatchFrame) -> bool {
        let mut applied = false;
        for action in &frame.actions {
            if action.trigger == UiNodeEventTrigger::Click {
                match action.action_id.as_str() {
                    IN_GAME_EDITOR_TOGGLE_ACTION => {
                        self.toggle_in_game_editor();
                        applied = true;
                        continue;
                    }
                    IN_GAME_EDITOR_CLOSE_ACTION => {
                        self.set_in_game_editor_enabled(false);
                        applied = true;
                        continue;
                    }
                    _ => {}
                }
            }

            if !self.in_game_editor_enabled() || action.trigger != UiNodeEventTrigger::ValueChanged
            {
                continue;
            }
            let Some(field) = TransformEditField::parse(action.action_id.as_str()) else {
                continue;
            };
            let Some(value) = action_payload_f32(&action.payload) else {
                continue;
            };
            if self.apply_selected_transform_field(field, value) {
                applied = true;
            }
        }
        applied
    }

    fn apply_selected_transform_field(&self, field: TransformEditField, value: f32) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(entity) = self.selection() else {
            return false;
        };
        let changed = {
            let mut scene = self.scene.write();
            let Some(transform) = scene.world_mut().get_mut::<Transform>(entity) else {
                return false;
            };
            field.apply(transform, value)
        };
        if changed {
            self.publish_inspector_state(Some(entity));
            newengine_ulog_api::ulog::info!(
                "in-game editor: transform changed entity_key={} field={:?} value={:.4}",
                entity.stable_u64(),
                field,
                value,
            );
        }
        changed
    }

    fn select_entity_by_stable_key(&self, entity_key: u64) -> Option<EntityId> {
        let selected = {
            let scene = self.scene.read();
            let selected = scene
                .world()
                .iter_entities()
                .find(|entity| entity.stable_u64() == entity_key);
            selected
        };
        if let Some(entity) = selected {
            self.set_selection(Some(entity));
        }
        selected
    }
}

impl SceneBridge {
    pub(super) fn publish_in_game_editor_state(&self, enabled: bool) {
        let patch = UiStatePatch::new(0, GAME_HUD_SURFACE_ID)
            .with_change("ingame_editor", "enabled", serde_json::json!(enabled))
            .with_change(
                "ingame_editor",
                "mode_label",
                serde_json::json!(if enabled { "EDIT ON [F2]" } else { "EDIT [F2]" }),
            )
            .with_change(
                "ingame_editor",
                "hint",
                serde_json::json!(if enabled {
                    "Center reticle selects an object. Edit Transform on the right. F2 exits."
                } else {
                    "F2 opens the in-game object editor."
                }),
            );
        crate::ui_gateway::publish_state_patch(&patch, "engine.scene", IN_GAME_EDITOR_CONTRACT);
    }

    pub(super) fn publish_inspector_state(&self, selected: Option<EntityId>) {
        let snapshot = self.inspector_snapshot_json(selected);
        if self.in_game_editor_enabled() {
            publish_inspector_snapshot_to_surface(&snapshot, GAME_HUD_SURFACE_ID);
        } else {
            publish_inspector_snapshot_to_surface(&snapshot, EDITOR_INSPECTOR_SURFACE_ID);
        }
    }

    pub fn inspector_snapshot_json(&self, selected: Option<EntityId>) -> serde_json::Value {
        let Some(entity) = selected else {
            return serde_json::json!({
                "ok": true,
                "selected": false,
                "editable": false,
                "schema": INSPECTOR_CONTRACT,
                "entity": "",
                "entity_key": serde_json::Value::Null,
                "display_name": "No object under reticle",
                "position_x": "0.000",
                "position_y": "0.000",
                "position_z": "0.000",
                "rotation_x": "0.000",
                "rotation_y": "0.000",
                "rotation_z": "0.000",
                "scale_x": "1.000",
                "scale_y": "1.000",
                "scale_z": "1.000",
                "bounds_summary": "No Bounds component",
                "physics_summary": "No PhysicsBodyDesc component",
                "anchor_summary": "No SceneEntityAnchor component",
                "transform": serde_json::Value::Null,
                "bounds": serde_json::Value::Null,
                "physics_body": serde_json::Value::Null,
                "scene_anchor": serde_json::Value::Null,
                "repaired_reasons": [],
            });
        };

        let scene = self.scene.read();
        let world = scene.world();
        let transform_component = world.get::<Transform>(entity).copied();
        let bounds_component = world.get::<Bounds>(entity).copied();
        let physics_component = world
            .get::<crate::gameplay::PhysicsBodyDesc>(entity)
            .copied();
        let anchor_component = world.get::<crate::gameplay::SceneEntityAnchor>(entity);
        let transform = transform_component
            .as_ref()
            .map(transform_snapshot_json)
            .unwrap_or(serde_json::Value::Null);
        let bounds = bounds_component
            .as_ref()
            .map(bounds_snapshot_json)
            .unwrap_or(serde_json::Value::Null);
        let physics_body = physics_component
            .as_ref()
            .map(physics_body_snapshot_json)
            .unwrap_or(serde_json::Value::Null);
        let scene_anchor = anchor_component
            .map(scene_anchor_snapshot_json)
            .unwrap_or(serde_json::Value::Null);
        let entity_key = entity.stable_u64();
        let repaired_reasons = world
            .resource::<super::scene_object_validation::SceneObjectInvariantRuntimeDiagnostics>()
            .and_then(|diagnostics| {
                diagnostics
                    .last_report
                    .last_repaired_entities
                    .iter()
                    .rev()
                    .find(|record| record.entity_key == entity_key)
            })
            .map(|record| record.reasons.clone())
            .unwrap_or_default();
        let display_name = world
            .get::<newengine_scene::Name>(entity)
            .map(|name| name.as_str().to_owned())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("Entity {entity_key}"));
        let scalar = transform_component
            .map(transform_scalar_fields)
            .unwrap_or_else(TransformScalarFields::identity);

        serde_json::json!({
            "ok": true,
            "selected": true,
            "editable": transform_component.is_some(),
            "schema": INSPECTOR_CONTRACT,
            "entity": format!("{:?}", entity),
            "entity_key": entity_key,
            "display_name": display_name,
            "position_x": scalar.position_x,
            "position_y": scalar.position_y,
            "position_z": scalar.position_z,
            "rotation_x": scalar.rotation_x,
            "rotation_y": scalar.rotation_y,
            "rotation_z": scalar.rotation_z,
            "scale_x": scalar.scale_x,
            "scale_y": scalar.scale_y,
            "scale_z": scalar.scale_z,
            "bounds_summary": bounds_component.map(bounds_summary).unwrap_or_else(|| "No Bounds component".to_owned()),
            "physics_summary": physics_component.map(physics_summary).unwrap_or_else(|| "No PhysicsBodyDesc component".to_owned()),
            "anchor_summary": anchor_component.map(anchor_summary).unwrap_or_else(|| "No SceneEntityAnchor component".to_owned()),
            "transform": transform,
            "bounds": bounds,
            "physics_body": physics_body,
            "scene_anchor": scene_anchor,
            "repaired_reasons": repaired_reasons,
        })
    }
}
