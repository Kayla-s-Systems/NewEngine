use super::*;

impl SceneBridge {
    pub fn apply_editor_selection_actions(
        &self,
        frame: &UiEventDispatchFrame,
        additive: bool,
    ) -> bool {
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
            if let Some(entity) = self.entity_by_stable_key(entity_key) {
                if additive {
                    self.toggle_selection(entity);
                } else {
                    self.set_selection(Some(entity));
                }
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

    pub fn apply_editor_actor_actions(&self, frame: &UiEventDispatchFrame) -> bool {
        let mut applied = false;
        for action in &frame.actions {
            if action.trigger != UiNodeEventTrigger::Click {
                continue;
            }
            match action.action_id.as_str() {
                "editor.actor.duplicate" => {
                    applied |= !self.duplicate_selected_actors().is_empty();
                }
                "editor.actor.delete" => {
                    applied |= self.delete_selected_actors() > 0;
                }
                _ => {}
            }
        }
        applied
    }

    pub fn duplicate_selected_actors(&self) -> Vec<EntityId> {
        let selected = self.selections();
        if selected.is_empty() {
            return Vec::new();
        }

        let mut scene = self.scene.write();
        let world = scene.world_mut();
        let protected_root = world
            .resource::<newengine_scene::SceneState>()
            .and_then(|state| state.root);
        let mut duplicated = Vec::<(EntityId, EntityId, Option<u64>)>::new();

        macro_rules! clone_component {
            ($source:expr, $target:expr, $ty:ty) => {
                if let Some(value) = world.get::<$ty>($source).cloned() {
                    let _ = world.insert($target, value);
                }
            };
        }

        for source in selected {
            if !world.exists(source)
                || protected_root == Some(source)
                || world
                    .get::<crate::editor_viewport::EditorGizmoAxisComponent>(source)
                    .is_some()
                || world.get::<crate::gameplay::PlayerActor>(source).is_some()
            {
                continue;
            }

            let name = world
                .get::<newengine_scene::components::Name>(source)
                .map(|name| name.0.clone())
                .unwrap_or_else(|| format!("Actor {}", source.stable_u64()));
            let parent_key = world
                .get::<newengine_transform_api::Parent>(source)
                .map(|parent| parent.0.stable_id);
            let target = newengine_scene::spawn_named(world, format!("{name} Copy"));

            clone_component!(source, target, Transform);
            clone_component!(source, target, Bounds);
            clone_component!(source, target, Primitive);
            clone_component!(source, target, MaterialRef);
            clone_component!(
                source,
                target,
                newengine_model_domain_api::MeshRenderOptions
            );
            clone_component!(source, target, PhysicsBodyDesc);
            clone_component!(source, target, DisplayVisibility);
            clone_component!(source, target, DirectionalLight);
            clone_component!(source, target, PointLight);
            clone_component!(source, target, newengine_lighting::SpotLight);
            clone_component!(
                source,
                target,
                newengine_procedural_noise::ProceduralTerrain
            );
            clone_component!(source, target, SceneImportedAssetDescriptor);
            clone_component!(source, target, crate::gameplay::ModelRenderComponent);
            clone_component!(source, target, crate::AudioEmitter);
            clone_component!(source, target, crate::AcousticSurface);
            clone_component!(source, target, crate::AudioEnvironmentZone);
            clone_component!(source, target, crate::AudioPortal);
            clone_component!(source, target, crate::AudioAmbienceBed);
            clone_component!(source, target, crate::gameplay::GameplayActor);
            clone_component!(source, target, crate::gameplay::SceneEntityAnchor);
            clone_component!(source, target, DefinitionInstance);

            duplicated.push((source, target, parent_key));
        }

        let remap = duplicated
            .iter()
            .map(|(source, target, _)| (source.stable_u64(), *target))
            .collect::<std::collections::BTreeMap<_, _>>();
        for (_, target, parent_key) in &duplicated {
            let Some(parent_key) = *parent_key else {
                continue;
            };
            let parent = remap.get(&parent_key).copied().or_else(|| {
                world
                    .iter_entities()
                    .find(|entity| entity.stable_u64() == parent_key)
            });
            let _ = newengine_transform::set_parent(world, *target, parent);
        }

        let new_selection = duplicated
            .iter()
            .map(|(_, target, _)| *target)
            .collect::<Vec<_>>();
        drop(scene);
        self.replace_selections(new_selection.iter().copied());
        if !new_selection.is_empty() {
            newengine_ulog_api::ulog::info!(
                "editor actor duplicate: duplicated={} selection_count={}",
                new_selection.len(),
                new_selection.len()
            );
        }
        new_selection
    }

    pub fn delete_selected_actors(&self) -> usize {
        let selected = self.selections();
        if selected.is_empty() {
            return 0;
        }
        let mut scene = self.scene.write();
        let world = scene.world_mut();
        let protected_root = world
            .resource::<newengine_scene::SceneState>()
            .and_then(|state| state.root);
        let selected_keys = selected
            .iter()
            .filter(|entity| protected_root != Some(**entity))
            .map(|entity| entity.stable_u64())
            .collect::<std::collections::BTreeSet<_>>();
        if selected_keys.is_empty() {
            return 0;
        }

        let children_to_detach = world
            .iter_entities()
            .filter(|entity| {
                world
                    .get::<newengine_transform_api::Parent>(*entity)
                    .is_some_and(|parent| selected_keys.contains(&parent.0.stable_id))
                    && !selected_keys.contains(&entity.stable_u64())
            })
            .collect::<Vec<_>>();
        for child in children_to_detach {
            let _ = newengine_transform::set_parent(world, child, None);
        }

        let mut deleted = 0usize;
        for entity in selected {
            if protected_root == Some(entity) || !world.exists(entity) {
                continue;
            }
            if world
                .get::<crate::editor_viewport::EditorGizmoAxisComponent>(entity)
                .is_some()
            {
                continue;
            }
            let _ = world.despawn(entity);
            deleted += 1;
        }

        if let Some(state) = world.resource_mut::<newengine_scene::SceneState>() {
            if state
                .active_camera
                .is_some_and(|camera| selected_keys.contains(&camera.stable_u64()))
            {
                state.active_camera = None;
            }
        }
        drop(scene);
        self.replace_selections(std::iter::empty());
        if deleted > 0 {
            newengine_ulog_api::ulog::info!("editor actor delete: deleted={deleted}");
        }
        deleted
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

    fn entity_by_stable_key(&self, entity_key: u64) -> Option<EntityId> {
        let scene = self.scene.read();
        let entity = scene
            .world()
            .iter_entities()
            .find(|entity| entity.stable_u64() == entity_key);
        entity
    }
    pub fn editor_scene_snapshots(
        &self,
        frame_index: u64,
    ) -> (UiEditorSceneSnapshot, UiEditorInspectorSnapshot) {
        let selected = self.selections();
        let selected_keys = selected
            .iter()
            .map(|entity| entity.stable_u64())
            .collect::<Vec<_>>();
        let primary = self.selection();
        let scene = self.scene.read();
        let world = scene.world();
        let mut entities = Vec::new();

        for (entity, name) in world.query::<newengine_scene::components::Name>() {
            if world
                .get::<crate::editor_viewport::EditorGizmoAxisComponent>(entity)
                .is_some()
            {
                continue;
            }
            let mut components = Vec::new();
            if world.get::<Transform>(entity).is_some() {
                components.push("Transform".to_owned());
            }
            if world.get::<Primitive>(entity).is_some() {
                components.push("Static Mesh".to_owned());
            }
            if world.get::<PhysicsBodyDesc>(entity).is_some() {
                components.push("Collision".to_owned());
            }
            if world.get::<DirectionalLight>(entity).is_some() {
                components.push("Directional Light".to_owned());
            }
            if world.get::<PointLight>(entity).is_some() {
                components.push("Point Light".to_owned());
            }
            if world.get::<newengine_lighting::SpotLight>(entity).is_some() {
                components.push("Spot Light".to_owned());
            }
            if world
                .get::<newengine_procedural_noise::ProceduralTerrain>(entity)
                .is_some()
            {
                components.push("Terrain".to_owned());
            }
            if world.get::<crate::gameplay::PlayerActor>(entity).is_some() {
                components.push("Player".to_owned());
            }
            if world.get::<SceneImportedAssetDescriptor>(entity).is_some() {
                components.push("Imported Asset".to_owned());
            }
            if world
                .get::<crate::gameplay::ModelRenderComponent>(entity)
                .is_some()
            {
                components.push("Model Render".to_owned());
            }
            if world.get::<crate::AudioEmitter>(entity).is_some() {
                components.push("Audio Emitter".to_owned());
            }
            if world.get::<crate::AcousticSurface>(entity).is_some() {
                components.push("Acoustic Surface".to_owned());
            }
            if world.get::<crate::AudioEnvironmentZone>(entity).is_some() {
                components.push("Audio Environment Zone".to_owned());
            }
            if world.get::<crate::AudioPortal>(entity).is_some() {
                components.push("Audio Portal".to_owned());
            }
            if world.get::<crate::AudioAmbienceBed>(entity).is_some() {
                components.push("Audio Ambience Bed".to_owned());
            }

            let kind = if world.get::<DirectionalLight>(entity).is_some() {
                "Directional Light"
            } else if world.get::<PointLight>(entity).is_some() {
                "Point Light"
            } else if world.get::<newengine_lighting::SpotLight>(entity).is_some() {
                "Spot Light"
            } else if world.get::<crate::AudioEnvironmentZone>(entity).is_some() {
                "Audio Environment Zone Actor"
            } else if world.get::<crate::AudioPortal>(entity).is_some() {
                "Audio Portal Actor"
            } else if world.get::<crate::AudioAmbienceBed>(entity).is_some() {
                "Audio Ambience Bed Actor"
            } else if world.get::<crate::AudioEmitter>(entity).is_some() {
                "Audio Emitter Actor"
            } else if world
                .get::<newengine_procedural_noise::ProceduralTerrain>(entity)
                .is_some()
            {
                "Terrain"
            } else if world.get::<crate::gameplay::PlayerActor>(entity).is_some() {
                "Player"
            } else if world.get::<SceneImportedAssetDescriptor>(entity).is_some() {
                "Static Mesh Actor"
            } else if world.get::<Primitive>(entity).is_some() {
                "Primitive Actor"
            } else {
                "Actor"
            };

            let parent_key = world
                .get::<newengine_transform_api::Parent>(entity)
                .map(|parent| parent.0.stable_id);
            entities.push(UiEditorSceneEntitySnapshot {
                entity_key: entity.stable_u64(),
                parent_key,
                name: name.0.clone(),
                kind: kind.to_owned(),
                selected: selected.contains(&entity),
                components,
            });
        }
        entities.sort_by(|a, b| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
                .then(a.entity_key.cmp(&b.entity_key))
        });

        let scene_snapshot = UiEditorSceneSnapshot {
            version: 1,
            frame_index,
            entities,
            selected_keys: selected_keys.clone(),
        };

        let inspector = if let Some(entity) = primary {
            let name = world
                .get::<newengine_scene::components::Name>(entity)
                .map(|name| name.0.clone())
                .unwrap_or_else(|| format!("Entity {}", entity.stable_u64()));
            let mut components = Vec::new();
            if world.get::<Transform>(entity).is_some() {
                components.push("Transform".to_owned());
            }
            if world.get::<Primitive>(entity).is_some() {
                components.push("Static Mesh".to_owned());
            }
            if world.get::<PhysicsBodyDesc>(entity).is_some() {
                components.push("Collision".to_owned());
            }
            if world.get::<DirectionalLight>(entity).is_some() {
                components.push("Directional Light".to_owned());
            }
            if world.get::<PointLight>(entity).is_some() {
                components.push("Point Light".to_owned());
            }
            if world.get::<newengine_lighting::SpotLight>(entity).is_some() {
                components.push("Spot Light".to_owned());
            }
            if world
                .get::<newengine_procedural_noise::ProceduralTerrain>(entity)
                .is_some()
            {
                components.push("Terrain".to_owned());
            }
            if world.get::<crate::gameplay::PlayerActor>(entity).is_some() {
                components.push("Player".to_owned());
            }
            if world.get::<SceneImportedAssetDescriptor>(entity).is_some() {
                components.push("Imported Asset".to_owned());
            }
            if world
                .get::<crate::gameplay::ModelRenderComponent>(entity)
                .is_some()
            {
                components.push("Model Render".to_owned());
            }
            if world.get::<crate::AudioEmitter>(entity).is_some() {
                components.push("Audio Emitter".to_owned());
            }

            let kind = scene_snapshot
                .entities
                .iter()
                .find(|item| item.entity_key == entity.stable_u64())
                .map(|item| item.kind.clone())
                .unwrap_or_else(|| "Actor".to_owned());
            let transform = world.get::<Transform>(entity).map(|transform| {
                let (yaw, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
                UiEditorInspectorTransformSnapshot {
                    position: [
                        transform.position.x,
                        transform.position.y,
                        transform.position.z,
                    ],
                    rotation_degrees: [yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees()],
                    scale: [transform.scale.x, transform.scale.y, transform.scale.z],
                }
            });
            UiEditorInspectorSnapshot {
                version: 1,
                frame_index,
                entity_key: Some(entity.stable_u64()),
                name,
                kind,
                selection_count: selected_keys.len(),
                transform,
                components,
            }
        } else {
            UiEditorInspectorSnapshot {
                frame_index,
                ..UiEditorInspectorSnapshot::default()
            }
        };

        (scene_snapshot, inspector)
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

    pub(crate) fn refresh_editor_inspector(&self) {
        self.publish_inspector_state(self.selection());
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
