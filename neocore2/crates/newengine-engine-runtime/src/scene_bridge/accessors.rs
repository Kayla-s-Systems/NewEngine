use super::*;
use newengine_bounds::Bounds;
use newengine_ui_api::{UiEventDispatchFrame, UiNodeEventTrigger, UiStatePatch};

impl SceneBridge {
    #[inline]
    pub fn register_imported_asset_assembler(&self, assembler: SceneImportedAssetAssembler) {
        let mut registry = self.asset_assemblers.write();
        if let Some(existing) = registry.iter_mut().find(|it| it.key == assembler.key) {
            *existing = assembler;
            return;
        }
        registry.push(assembler);
    }

    #[inline]
    pub fn imported_asset_assemblers_snapshot(&self) -> Vec<SceneImportedAssetAssembler> {
        self.asset_assemblers.read().clone()
    }

    #[inline]
    pub fn authority_bridge(
        &self,
    ) -> std::sync::Arc<crate::authority::RuntimeWorldAuthorityBridge> {
        std::sync::Arc::clone(&self.authority)
    }

    #[inline]
    pub fn authority_snapshot(
        &self,
    ) -> newengine_runtime_host::world_authority::WorldAuthoritySnapshot {
        self.authority.detect()
    }

    #[inline]
    pub fn selection(&self) -> Option<EntityId> {
        *self.selection.lock()
    }

    #[inline]
    pub fn selection_authority_handle(&self) -> Option<newengine_entity_api::EntityHandle> {
        *self.selection_authority.lock()
    }

    #[inline]
    pub fn set_selection(&self, id: Option<EntityId>) {
        *self.selection.lock() = id;
        let authority = id.and_then(|entity| {
            let scene = self.scene.read();
            crate::authority::current_entity_authority_map(scene.world())
                .and_then(|map| map.provider_for_native(entity))
        });
        *self.selection_authority.lock() = authority;
        self.publish_inspector_state(id);
    }

    #[inline]
    pub fn play_mode(&self) -> GameRunMode {
        *self.play_mode.lock()
    }

    #[inline]
    pub fn materials_snapshot(&self) -> Vec<(String, MaterialId)> {
        let reg = self.materials.read();
        let mut out: Vec<(String, MaterialId)> = reg
            .snapshot()
            .into_iter()
            .map(|it| (it.name, it.id))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[inline]
    pub fn primitives_snapshot(&self) -> Vec<(String, PrimitiveId)> {
        let reg = self.primitives.read();
        let mut out: Vec<(String, PrimitiveId)> = reg
            .ids()
            .filter_map(|id| reg.name(id).map(|n| (n.to_string(), id)))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

impl SceneBridge {
    pub fn apply_inventory_ui_actions(&self, frame: &UiEventDispatchFrame) -> bool {
        let mut scene = self.scene.write();
        crate::gameplay::apply_inventory_ui_actions(scene.world_mut(), frame)
    }

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
    fn publish_inspector_state(&self, selected: Option<EntityId>) {
        let snapshot = self.inspector_snapshot_json(selected);
        let Some(object) = snapshot.as_object() else {
            return;
        };
        let mut patch = UiStatePatch::new(0, "engine.ui.editor.inspector");
        for (key, value) in object {
            patch = patch.with_change("selected_entity", key.clone(), value.clone());
        }
        crate::ui_gateway::publish_state_patch(
            &patch,
            "engine.scene",
            "newengine.scene.selected_entity_inspector.snapshot.v1",
        );
    }

    pub fn inspector_snapshot_json(&self, selected: Option<EntityId>) -> serde_json::Value {
        let Some(entity) = selected else {
            return serde_json::json!({
                "ok": true,
                "selected": false,
                "schema": "newengine.scene.selected_entity_inspector.snapshot.v1",
                "entity": "",
                "entity_key": serde_json::Value::Null,
                "transform": serde_json::Value::Null,
                "bounds": serde_json::Value::Null,
                "physics_body": serde_json::Value::Null,
                "scene_anchor": serde_json::Value::Null,
                "repaired_reasons": [],
            });
        };

        let scene = self.scene.read();
        let world = scene.world();
        let transform = world
            .get::<Transform>(entity)
            .map(transform_snapshot_json)
            .unwrap_or(serde_json::Value::Null);
        let bounds = world
            .get::<Bounds>(entity)
            .map(bounds_snapshot_json)
            .unwrap_or(serde_json::Value::Null);
        let physics_body = world
            .get::<crate::gameplay::PhysicsBodyDesc>(entity)
            .map(physics_body_snapshot_json)
            .unwrap_or(serde_json::Value::Null);
        let scene_anchor = world
            .get::<crate::gameplay::SceneEntityAnchor>(entity)
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

        serde_json::json!({
            "ok": true,
            "selected": true,
            "schema": "newengine.scene.selected_entity_inspector.snapshot.v1",
            "entity": format!("{:?}", entity),
            "entity_key": entity_key,
            "transform": transform,
            "bounds": bounds,
            "physics_body": physics_body,
            "scene_anchor": scene_anchor,
            "repaired_reasons": repaired_reasons,
        })
    }
}

fn transform_snapshot_json(transform: &Transform) -> serde_json::Value {
    serde_json::json!({
        "position": [transform.position.x, transform.position.y, transform.position.z],
        "rotation": format!("{:?}", transform.rotation),
        "scale": [transform.scale.x, transform.scale.y, transform.scale.z],
    })
}

fn bounds_snapshot_json(bounds: &Bounds) -> serde_json::Value {
    let local_center = bounds.local_aabb.center();
    let local_half = bounds.local_aabb.half_extents();
    let world_center = bounds.world_aabb.center();
    let world_half = bounds.world_aabb.half_extents();
    serde_json::json!({
        "kind": format!("{:?}", bounds.kind),
        "local_center": [local_center.x, local_center.y, local_center.z],
        "local_half_extents": [local_half.x, local_half.y, local_half.z],
        "world_center": [world_center.x, world_center.y, world_center.z],
        "world_half_extents": [world_half.x, world_half.y, world_half.z],
    })
}

fn physics_body_snapshot_json(body: &crate::gameplay::PhysicsBodyDesc) -> serde_json::Value {
    serde_json::json!({
        "kind": format!("{:?}", body.kind),
        "shape": format!("{:?}", body.shape),
        "is_trigger": body.flags.is_trigger,
        "participates_in_queries": body.flags.participates_in_queries,
        "casts_contacts": body.flags.casts_contacts,
        "material": {
            "friction": body.material.friction,
            "restitution": body.material.restitution,
            "density": body.material.density,
        },
    })
}

fn scene_anchor_snapshot_json(anchor: &crate::gameplay::SceneEntityAnchor) -> serde_json::Value {
    serde_json::json!({
        "role": format!("{:?}", anchor.role),
        "label": anchor.label,
    })
}

fn selection_entity_key_from_action(action_id: &str, payload: &serde_json::Value) -> Option<u64> {
    const PREFIX: &str = "engine.editor.selection.select_entity";
    let action_id = action_id.trim();
    if let Some(suffix) = action_id.strip_prefix(&(PREFIX.to_owned() + ".")) {
        if let Ok(value) = suffix.trim().parse::<u64>() {
            return Some(value);
        }
    }
    if action_id != PREFIX {
        return None;
    }
    payload
        .get("entity_key")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| {
            payload
                .get("entity")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| value.trim().parse::<u64>().ok())
        })
}
