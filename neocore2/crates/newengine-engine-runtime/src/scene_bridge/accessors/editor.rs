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
                    IN_GAME_EDITOR_SAVE_ACTION if self.in_game_editor_enabled() => {
                        match self.save_authored_project_world() {
                            Ok(count) => newengine_ulog_api::ulog::info!(
                                "in-game editor: project save complete placements={count}"
                            ),
                            Err(error) => newengine_ulog_api::ulog::error!(
                                "in-game editor: project save failed err='{}'",
                                error
                            ),
                        }
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
        let roots = canonical_editor_actor_roots(world, &selected, protected_root);
        let mut duplicated_roots = Vec::new();

        macro_rules! clone_component {
            ($source:expr, $target:expr, $ty:ty) => {
                if let Some(value) = world.get::<$ty>($source).cloned() {
                    let _ = world.insert($target, value);
                }
            };
        }

        for source_root in roots {
            let source_authored = world
                .get::<crate::gameplay::AuthoredMapPlacement>(source_root)
                .cloned()
                .filter(|authored| authored.primary);
            let duplicate_authored = source_authored
                .as_ref()
                .and_then(|authored| self.prepare_authored_duplicate(world, source_root, authored));

            let mut clone_roots = vec![source_root];
            if let Some(authored) = source_authored.as_ref() {
                clone_roots.extend(
                    world
                        .query::<crate::gameplay::AuthoredMapPlacement>()
                        .filter_map(|(entity, candidate)| {
                            (!candidate.primary
                                && candidate.map_ref == authored.map_ref
                                && candidate.placement_id == authored.placement_id
                                && candidate.source == authored.source)
                                .then_some(entity)
                        }),
                );
            }
            clone_roots.sort_by_key(|entity| entity.stable_u64());
            clone_roots.dedup();

            let sources = collect_editor_actor_subtree(world, &clone_roots);
            let root_keys = clone_roots
                .iter()
                .map(|entity| entity.stable_u64())
                .collect::<std::collections::BTreeSet<_>>();
            let mut remap = std::collections::BTreeMap::<u64, EntityId>::new();

            for source in &sources {
                let name = world
                    .get::<newengine_scene::components::Name>(*source)
                    .map(|name| name.0.clone())
                    .unwrap_or_else(|| format!("Actor {}", source.stable_u64()));
                let copied_name = if root_keys.contains(&source.stable_u64()) {
                    format!("{name} Copy")
                } else {
                    name
                };
                let target = newengine_scene::spawn_named(world, copied_name);

                clone_component!(*source, target, Transform);
                clone_component!(*source, target, Bounds);
                clone_component!(*source, target, Primitive);
                clone_component!(*source, target, MaterialRef);
                clone_component!(
                    *source,
                    target,
                    newengine_model_domain_api::MeshRenderOptions
                );
                clone_component!(*source, target, PhysicsBodyDesc);
                clone_component!(*source, target, crate::gameplay::StaticMeshCollider);
                clone_component!(*source, target, crate::gameplay::PhysicsSurface);
                clone_component!(*source, target, DisplayVisibility);
                clone_component!(*source, target, DirectionalLight);
                clone_component!(*source, target, PointLight);
                clone_component!(*source, target, newengine_lighting::SpotLight);
                clone_component!(
                    *source,
                    target,
                    newengine_procedural_noise::ProceduralTerrain
                );
                clone_component!(*source, target, SceneImportedAssetDescriptor);
                clone_component!(*source, target, PrimitiveMaterialBase);
                clone_component!(*source, target, crate::gameplay::ModelRenderComponent);
                clone_component!(*source, target, crate::AudioEmitter);
                clone_component!(*source, target, crate::AcousticSurface);
                clone_component!(*source, target, crate::AudioEnvironmentZone);
                clone_component!(*source, target, crate::AudioPortal);
                clone_component!(*source, target, crate::AudioAmbienceBed);
                clone_component!(*source, target, crate::gameplay::GameplayActor);
                clone_component!(*source, target, crate::gameplay::SceneEntityAnchor);
                clone_component!(*source, target, DefinitionInstance);
                clone_component!(*source, target, newengine_sim::Velocity);
                clone_component!(*source, target, newengine_sim::AngularVelocity);
                clone_component!(
                    *source,
                    target,
                    crate::gameplay::AuthoredMapPlacementReplicaScaleState
                );

                if let (
                    Some(original_identity),
                    Some((new_primary_identity, clone_origin)),
                    Some(source_identity),
                ) = (
                    source_authored.as_ref(),
                    duplicate_authored.as_ref(),
                    world
                        .get::<crate::gameplay::AuthoredMapPlacement>(*source)
                        .cloned(),
                ) {
                    if source_identity.map_ref == original_identity.map_ref
                        && source_identity.placement_id == original_identity.placement_id
                        && source_identity.source == original_identity.source
                    {
                        let _ = world.insert(
                            target,
                            crate::gameplay::AuthoredMapPlacement::new(
                                new_primary_identity.map_ref.clone(),
                                new_primary_identity.placement_id.clone(),
                                new_primary_identity.source,
                                source_identity.primary,
                            ),
                        );
                        if source_identity.primary {
                            let _ = world.insert(target, clone_origin.clone());
                            let _ =
                                world.insert(target, crate::gameplay::AuthoredMapPlacementDirty);
                        }
                    }
                }

                remap.insert(source.stable_u64(), target);
            }

            for source in &sources {
                let Some(target) = remap.get(&source.stable_u64()).copied() else {
                    continue;
                };
                let parent_key = world
                    .get::<newengine_transform_api::Parent>(*source)
                    .map(|parent| parent.0.stable_id);
                let parent = parent_key.and_then(|parent_key| {
                    remap.get(&parent_key).copied().or_else(|| {
                        world
                            .iter_entities()
                            .find(|entity| entity.stable_u64() == parent_key)
                    })
                });
                let _ = newengine_transform::set_parent(world, target, parent);
            }

            if let Some(target_root) = remap.get(&source_root.stable_u64()).copied() {
                duplicated_roots.push(target_root);
            }
        }

        drop(scene);
        self.replace_selections(duplicated_roots.iter().copied());
        if !duplicated_roots.is_empty() {
            newengine_ulog_api::ulog::info!(
                "editor actor duplicate: actors={} deep_clone=true authored_create_journal=true",
                duplicated_roots.len(),
            );
        }
        duplicated_roots
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
        let actor_roots = canonical_editor_actor_roots(world, &selected, protected_root);
        if actor_roots.is_empty() {
            return 0;
        }

        let mut delete_roots = actor_roots.clone();
        let mut authored_deletions = Vec::new();
        for root in &actor_roots {
            let Some(authored) = world
                .get::<crate::gameplay::AuthoredMapPlacement>(*root)
                .cloned()
                .filter(|authored| authored.primary)
            else {
                continue;
            };

            if world
                .get::<crate::gameplay::AuthoredMapPlacementCloneSource>(*root)
                .is_none()
            {
                authored_deletions.push(authored.clone());
            }
            delete_roots.extend(
                world
                    .query::<crate::gameplay::AuthoredMapPlacement>()
                    .filter_map(|(entity, candidate)| {
                        (!candidate.primary
                            && candidate.map_ref == authored.map_ref
                            && candidate.placement_id == authored.placement_id
                            && candidate.source == authored.source)
                            .then_some(entity)
                    }),
            );
        }
        delete_roots.sort_by_key(|entity| entity.stable_u64());
        delete_roots.dedup();

        let mut deletion_order = collect_editor_actor_subtree(world, &delete_roots)
            .into_iter()
            .map(|entity| {
                (
                    editor_entity_depth_in_set(world, entity, &delete_roots),
                    entity,
                )
            })
            .collect::<Vec<_>>();
        deletion_order.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.stable_u64().cmp(&a.1.stable_u64()))
        });
        let deleted_keys = deletion_order
            .iter()
            .map(|(_, entity)| entity.stable_u64())
            .collect::<std::collections::BTreeSet<_>>();

        if let Some(state) = world.resource_mut::<newengine_scene::SceneState>() {
            if state
                .active_camera
                .is_some_and(|camera| deleted_keys.contains(&camera.stable_u64()))
            {
                state.active_camera = None;
            }
        }

        let mut deleted_entities = 0usize;
        for (_, entity) in deletion_order {
            if world.exists(entity) {
                let _ = world.despawn(entity);
                deleted_entities = deleted_entities.saturating_add(1);
            }
        }
        drop(scene);

        for authored in authored_deletions {
            self.record_authored_deletion(&authored);
        }
        self.replace_selections(std::iter::empty());
        newengine_ulog_api::ulog::info!(
            "editor actor delete: actors={} entities={} authored_delete_journal=true",
            actor_roots.len(),
            deleted_entities,
        );
        actor_roots.len()
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
            {
                let mut scene = self.scene.write();
                let world = scene.world_mut();
                let _ = world.insert(entity, crate::gameplay::AuthoredMapPlacementDirty);
                crate::editor_viewport::sync_authored_map_placement_replicas(world, entity);
            }
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

fn canonical_editor_actor_roots(
    world: &newengine_ecs::World,
    selected: &[EntityId],
    protected_root: Option<EntityId>,
) -> Vec<EntityId> {
    let mut roots = selected
        .iter()
        .copied()
        .filter(|entity| world.exists(*entity))
        .map(|entity| authored_editor_actor_root(world, entity).unwrap_or(entity))
        .filter(|entity| {
            protected_root != Some(*entity)
                && world
                    .get::<crate::editor_viewport::EditorGizmoAxisComponent>(*entity)
                    .is_none()
                && world.get::<crate::gameplay::PlayerActor>(*entity).is_none()
        })
        .collect::<Vec<_>>();
    roots.sort_by_key(|entity| entity.stable_u64());
    roots.dedup();

    let root_keys = roots
        .iter()
        .map(|entity| entity.stable_u64())
        .collect::<std::collections::BTreeSet<_>>();
    roots.retain(|entity| {
        let mut cursor = world
            .get::<newengine_transform_api::Parent>(*entity)
            .map(|parent| parent.0.stable_id);
        let mut depth = 0usize;
        while let Some(parent_key) = cursor {
            if root_keys.contains(&parent_key) {
                return false;
            }
            let Some(parent) = world
                .iter_entities()
                .find(|candidate| candidate.stable_u64() == parent_key)
            else {
                break;
            };
            cursor = world
                .get::<newengine_transform_api::Parent>(parent)
                .map(|next| next.0.stable_id);
            depth += 1;
            if depth >= 128 {
                break;
            }
        }
        true
    });
    roots
}

fn authored_editor_actor_root(world: &newengine_ecs::World, entity: EntityId) -> Option<EntityId> {
    let mut cursor = Some(entity);
    let mut depth = 0usize;
    while let Some(current) = cursor {
        if world.get::<crate::gameplay::PlayerActor>(current).is_some() {
            return Some(current);
        }
        if let Some(authored) = world.get::<crate::gameplay::AuthoredMapPlacement>(current) {
            if authored.primary {
                return Some(current);
            }
            if let Some(primary) = world
                .query::<crate::gameplay::AuthoredMapPlacement>()
                .find_map(|(candidate, identity)| {
                    (identity.primary
                        && identity.map_ref == authored.map_ref
                        && identity.placement_id == authored.placement_id
                        && identity.source == authored.source)
                        .then_some(candidate)
                })
            {
                return Some(primary);
            }
        }
        let parent_key = world
            .get::<newengine_transform_api::Parent>(current)
            .map(|parent| parent.0.stable_id);
        cursor = parent_key.and_then(|key| {
            world
                .iter_entities()
                .find(|candidate| candidate.stable_u64() == key)
        });
        depth += 1;
        if depth >= 128 {
            break;
        }
    }
    None
}

fn collect_editor_actor_subtree(world: &newengine_ecs::World, roots: &[EntityId]) -> Vec<EntityId> {
    let mut keys = roots
        .iter()
        .filter(|entity| world.exists(**entity))
        .map(|entity| entity.stable_u64())
        .collect::<std::collections::BTreeSet<_>>();
    loop {
        let mut changed = false;
        for entity in world.iter_entities() {
            if keys.contains(&entity.stable_u64()) {
                continue;
            }
            if world
                .get::<newengine_transform_api::Parent>(entity)
                .is_some_and(|parent| keys.contains(&parent.0.stable_id))
            {
                changed |= keys.insert(entity.stable_u64());
            }
        }
        if !changed {
            break;
        }
    }

    let mut entities = world
        .iter_entities()
        .filter(|entity| keys.contains(&entity.stable_u64()))
        .collect::<Vec<_>>();
    entities.sort_by_key(|entity| entity.stable_u64());
    entities
}

fn editor_entity_depth_in_set(
    world: &newengine_ecs::World,
    entity: EntityId,
    roots: &[EntityId],
) -> usize {
    let root_keys = roots
        .iter()
        .map(|root| root.stable_u64())
        .collect::<std::collections::BTreeSet<_>>();
    let mut depth = 0usize;
    let mut cursor = Some(entity);
    while let Some(current) = cursor {
        if root_keys.contains(&current.stable_u64()) {
            return depth;
        }
        let parent_key = world
            .get::<newengine_transform_api::Parent>(current)
            .map(|parent| parent.0.stable_id);
        cursor = parent_key.and_then(|key| {
            world
                .iter_entities()
                .find(|candidate| candidate.stable_u64() == key)
        });
        depth += 1;
        if depth >= 128 {
            break;
        }
    }
    depth
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
                    "World Editor: hold RMB for WASD/Q/E free-fly (Shift boost); release RMB for Q/W/E/R tools; Ctrl+S save; F2 exit."
                } else {
                    "F2 opens the World Editor with free-fly, noclip and authoring tools."
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
