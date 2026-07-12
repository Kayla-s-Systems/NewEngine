use super::*;
use newengine_bounds::Bounds;
use newengine_ui_api::{UiEventDispatchFrame, UiNodeEventTrigger, UiStatePatch};

const EDITOR_INSPECTOR_SURFACE_ID: &str = "engine.ui.editor.inspector";
const GAME_HUD_SURFACE_ID: &str = "game.hud";
const INSPECTOR_CONTRACT: &str = "newengine.scene.selected_entity_inspector.snapshot.v1";
const IN_GAME_EDITOR_CONTRACT: &str = "newengine.scene.ingame_editor.state.v1";
const IN_GAME_EDITOR_TOGGLE_ACTION: &str = "game.editor.toggle";
const IN_GAME_EDITOR_CLOSE_ACTION: &str = "game.editor.close";
const IN_GAME_EDITOR_TRANSFORM_PREFIX: &str = "game.editor.transform.";

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
        {
            let mut selection = self.selection.lock();
            if *selection == id {
                return;
            }
            *selection = id;
        }
        let authority = id.and_then(|entity| {
            let scene = self.scene.read();
            crate::authority::current_entity_authority_map(scene.world())
                .and_then(|map| map.provider_for_native(entity))
        });
        *self.selection_authority.lock() = authority;
        self.publish_inspector_state(id);
    }

    #[inline]
    pub fn in_game_editor_enabled(&self) -> bool {
        *self.in_game_editor_enabled.lock()
    }

    pub fn set_in_game_editor_enabled(&self, enabled: bool) -> bool {
        let changed = {
            let mut current = self.in_game_editor_enabled.lock();
            if *current == enabled {
                false
            } else {
                *current = enabled;
                true
            }
        };
        if !changed {
            return enabled;
        }
        if !enabled {
            self.set_selection(None);
        }
        self.publish_in_game_editor_state(enabled);
        if enabled {
            self.publish_inspector_state(self.selection());
        }
        newengine_ulog_api::ulog::info!(
            "in-game editor: mode={} source='engine.scene' center_pick={} gameplay_input_gated={}",
            if enabled { "edit" } else { "play" },
            enabled,
            enabled,
        );
        enabled
    }

    #[inline]
    pub fn toggle_in_game_editor(&self) -> bool {
        self.set_in_game_editor_enabled(!self.in_game_editor_enabled())
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
    fn publish_in_game_editor_state(&self, enabled: bool) {
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

    fn publish_inspector_state(&self, selected: Option<EntityId>) {
        let snapshot = self.inspector_snapshot_json(selected);
        publish_inspector_snapshot_to_surface(&snapshot, EDITOR_INSPECTOR_SURFACE_ID);
        if self.in_game_editor_enabled() {
            publish_inspector_snapshot_to_surface(&snapshot, GAME_HUD_SURFACE_ID);
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

fn publish_inspector_snapshot_to_surface(snapshot: &serde_json::Value, surface_id: &str) {
    let Some(object) = snapshot.as_object() else {
        return;
    };
    let mut patch = UiStatePatch::new(0, surface_id);
    for (key, value) in object {
        patch = patch.with_change("selected_entity", key.clone(), value.clone());
    }
    crate::ui_gateway::publish_state_patch(&patch, "engine.scene", INSPECTOR_CONTRACT);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransformEditField {
    PositionX,
    PositionY,
    PositionZ,
    RotationX,
    RotationY,
    RotationZ,
    ScaleX,
    ScaleY,
    ScaleZ,
}

impl TransformEditField {
    fn parse(action_id: &str) -> Option<Self> {
        let suffix = action_id
            .trim()
            .strip_prefix(IN_GAME_EDITOR_TRANSFORM_PREFIX)?;
        Some(match suffix {
            "position.x" => Self::PositionX,
            "position.y" => Self::PositionY,
            "position.z" => Self::PositionZ,
            "rotation.x" => Self::RotationX,
            "rotation.y" => Self::RotationY,
            "rotation.z" => Self::RotationZ,
            "scale.x" => Self::ScaleX,
            "scale.y" => Self::ScaleY,
            "scale.z" => Self::ScaleZ,
            _ => return None,
        })
    }

    fn apply(self, transform: &mut Transform, value: f32) -> bool {
        let value = value.clamp(-1_000_000.0, 1_000_000.0);
        match self {
            Self::PositionX => transform.position.x = value,
            Self::PositionY => transform.position.y = value,
            Self::PositionZ => transform.position.z = value,
            Self::ScaleX => transform.scale.x = sanitize_scale(value),
            Self::ScaleY => transform.scale.y = sanitize_scale(value),
            Self::ScaleZ => transform.scale.z = sanitize_scale(value),
            Self::RotationX | Self::RotationY | Self::RotationZ => {
                let (mut yaw, mut pitch, mut roll) = transform.yaw_pitch_roll();
                match self {
                    Self::RotationX => pitch = value.to_radians(),
                    Self::RotationY => yaw = value.to_radians(),
                    Self::RotationZ => roll = value.to_radians(),
                    _ => unreachable!(),
                }
                transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
            }
        }
        true
    }
}

#[inline]
fn sanitize_scale(value: f32) -> f32 {
    if value.abs() < 0.001 {
        0.001_f32.copysign(if value == 0.0 { 1.0 } else { value })
    } else {
        value.clamp(-10_000.0, 10_000.0)
    }
}

fn action_payload_f32(payload: &serde_json::Value) -> Option<f32> {
    let value = payload.get("value").unwrap_or(payload);
    value
        .as_f64()
        .map(|value| value as f32)
        .or_else(|| value.as_str()?.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
}

struct TransformScalarFields {
    position_x: String,
    position_y: String,
    position_z: String,
    rotation_x: String,
    rotation_y: String,
    rotation_z: String,
    scale_x: String,
    scale_y: String,
    scale_z: String,
}

impl TransformScalarFields {
    fn identity() -> Self {
        Self {
            position_x: "0.000".to_owned(),
            position_y: "0.000".to_owned(),
            position_z: "0.000".to_owned(),
            rotation_x: "0.000".to_owned(),
            rotation_y: "0.000".to_owned(),
            rotation_z: "0.000".to_owned(),
            scale_x: "1.000".to_owned(),
            scale_y: "1.000".to_owned(),
            scale_z: "1.000".to_owned(),
        }
    }
}

fn transform_scalar_fields(transform: Transform) -> TransformScalarFields {
    let (yaw, pitch, roll) = transform.yaw_pitch_roll();
    TransformScalarFields {
        position_x: format!("{:.3}", transform.position.x),
        position_y: format!("{:.3}", transform.position.y),
        position_z: format!("{:.3}", transform.position.z),
        rotation_x: format!("{:.3}", pitch.to_degrees()),
        rotation_y: format!("{:.3}", yaw.to_degrees()),
        rotation_z: format!("{:.3}", roll.to_degrees()),
        scale_x: format!("{:.3}", transform.scale.x),
        scale_y: format!("{:.3}", transform.scale.y),
        scale_z: format!("{:.3}", transform.scale.z),
    }
}

fn transform_snapshot_json(transform: &Transform) -> serde_json::Value {
    let (yaw, pitch, roll) = transform.yaw_pitch_roll();
    serde_json::json!({
        "position": [transform.position.x, transform.position.y, transform.position.z],
        "rotation": format!("{:?}", transform.rotation),
        "rotation_degrees_xyz": [pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees()],
        "scale": [transform.scale.x, transform.scale.y, transform.scale.z],
    })
}

fn bounds_summary(bounds: Bounds) -> String {
    let center = bounds.world_aabb.center();
    let half = bounds.world_aabb.half_extents();
    format!(
        "{:?} center=({:.2}, {:.2}, {:.2}) half=({:.2}, {:.2}, {:.2})",
        bounds.kind, center.x, center.y, center.z, half.x, half.y, half.z
    )
}

fn physics_summary(body: crate::gameplay::PhysicsBodyDesc) -> String {
    format!(
        "{:?} / {:?} trigger={} query={} friction={:.2}",
        body.kind,
        body.shape,
        body.flags.is_trigger,
        body.flags.participates_in_queries,
        body.material.friction,
    )
}

fn anchor_summary(anchor: &crate::gameplay::SceneEntityAnchor) -> String {
    format!("{:?}: {}", anchor.role, anchor.label)
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

#[cfg(test)]
mod in_game_editor_tests {
    use super::*;

    #[test]
    fn parses_transform_action_fields() {
        assert_eq!(
            TransformEditField::parse("game.editor.transform.position.x"),
            Some(TransformEditField::PositionX)
        );
        assert_eq!(
            TransformEditField::parse("game.editor.transform.rotation.y"),
            Some(TransformEditField::RotationY)
        );
        assert!(TransformEditField::parse("game.editor.transform.unknown").is_none());
    }

    #[test]
    fn parses_numeric_and_text_action_values() {
        assert_eq!(
            action_payload_f32(&serde_json::json!({"value": 12.5})),
            Some(12.5)
        );
        assert_eq!(
            action_payload_f32(&serde_json::json!({"value": "-3.25"})),
            Some(-3.25)
        );
        assert!(action_payload_f32(&serde_json::json!({"value": "not-a-number"})).is_none());
    }

    #[test]
    fn transform_rotation_uses_xyz_degrees_contract() {
        let mut transform = Transform::default();
        TransformEditField::RotationY.apply(&mut transform, 90.0);
        let (yaw, pitch, roll) = transform.yaw_pitch_roll();
        assert!((yaw.to_degrees() - 90.0).abs() < 0.001);
        assert!(pitch.abs() < 0.001);
        assert!(roll.abs() < 0.001);
    }
}
