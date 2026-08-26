impl SceneBridge {
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
