impl SceneBridge {
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
}
