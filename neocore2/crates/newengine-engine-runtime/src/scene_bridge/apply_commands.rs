use super::*;
use super::definitions_runtime;
use newengine_model_domain_api::AssetGraphResolver;

impl SceneBridge {
    pub fn apply_commands(&self) {
        let cmds = {
            let mut q = self.queue.lock();
            if q.cmds.is_empty() {
                return;
            }
            std::mem::take(&mut q.cmds)
        };

        let mut pending_selection: Option<Option<EntityId>> = None;
        let mut next_mode: Option<GameRunMode> = None;

        let prims = self.primitives.read();
        let mats = self.materials.read();

        let default_mat = mats.register_named("Default", MaterialDescriptor::default());

        let mut scene = self.scene.write();

        for cmd in cmds {
            match cmd {
                SceneCommand::NewScene => {
                    *scene = Scene::new();
                    pending_selection = Some(reset_game_runtime_state(&mut *scene));
                    next_mode = Some(GameRunMode::Staging);
                }
                SceneCommand::LoadSceneAsset { asset } => {
                    *scene = Scene::new();
                    if let Err(e) = scene.load_asset(&asset) {
                        log::error!("scene.load_asset failed: {e}");
                    }
                    pending_selection = Some(reset_game_runtime_state(&mut *scene));
                    next_mode = Some(GameRunMode::Staging);
                }
                SceneCommand::SpawnPrimitive {
                    id,
                    name,
                    position,
                    scale,
                    color,
                } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let prim_index = world.query::<Primitive>().count();
                    let spawn_pos = place_spawn_position(
                        Vec3::new(position[0], position[1], position[2]),
                        prim_index,
                    );

                    let e = spawn_named(world, name);
                    let _ = newengine_transform::set_parent(world, e, Some(root));
                    let _ = world.insert(e, Primitive { id, color });

                    if let Some(bounds) = primitive_bounds(&prims, id) {
                        let _ = world.insert(e, bounds);
                    }

                    ensure_primitive_base(world, e, default_mat);
                    apply_primitive_instance(world, &*mats, e, default_mat, color);

                    if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                        t.position = spawn_pos;
                        t.scale = Vec3::new(scale[0], scale[1], scale[2]);
                    }

                    pending_selection = Some(Some(e));
                }
                SceneCommand::SpawnDirectionalLight {
                    name,
                    position,
                    direction_ws,
                } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let e = spawn_named(world, name);
                    let _ = newengine_transform::set_parent(world, e, Some(root));

                    let mut dl = DirectionalLight::default();
                    dl.direction_ws = direction_ws;
                    let _ = world.insert(e, dl);
                    if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                        t.position = Vec3::new(position[0], position[1], position[2]);
                    }
                    pending_selection = Some(Some(e));
                }
                SceneCommand::SpawnPointLight { name, position } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let e = spawn_named(world, name);
                    let _ = newengine_transform::set_parent(world, e, Some(root));
                    let _ = world.insert(e, PointLight::default());
                    if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                        t.position = Vec3::new(position[0], position[1], position[2]);
                    }
                    pending_selection = Some(Some(e));
                }
                SceneCommand::SpawnPlayer { name, position } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let player = spawn_default_player(
                        world,
                        Some(root),
                        name,
                        Vec3::new(position[0], position[1], position[2]),
                    );
                    pending_selection = Some(Some(player));
                }
                SceneCommand::SpawnImportedAsset { descriptor, name, position } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let e = spawn_named(world, name);
                    let _ = newengine_transform::set_parent(world, e, Some(root));

                    let assembler = resolve_asset_assembler(&self.asset_assemblers.read(), &descriptor);
                    let primitive_id = match assembler.assembly {
                        SceneImportedAssetAssemblyKind::StaticMeshActor => builtins::ID_CUBE,
                        SceneImportedAssetAssemblyKind::SceneAnchor => builtins::ID_PLANE,
                        SceneImportedAssetAssemblyKind::TextureCard => builtins::ID_PLANE,
                        SceneImportedAssetAssemblyKind::MaterialPreviewSphere => builtins::ID_SPHERE_UV,
                        SceneImportedAssetAssemblyKind::OpaqueProxy => imported_asset_primitive_id(&descriptor),
                    };
                    let _ = world.insert(e, Primitive {
                        id: primitive_id,
                        color: descriptor.tint,
                    });
                    let _ = world.insert(e, descriptor.clone());
                    let _ = world.insert(e, DisplayVisibility { mode: descriptor.assembly.display_mode });
                    if let Some(bounds) = primitive_bounds(&prims, primitive_id) {
                        let _ = world.insert(e, bounds);
                    }
                    ensure_primitive_base(world, e, default_mat);
                    apply_primitive_instance(world, &*mats, e, default_mat, descriptor.tint);
                    if let Some(collision) = imported_asset_collision(&descriptor) {
                        let _ = world.insert(e, collision);
                    }
                    if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                        t.position = Vec3::new(position[0], position[1], position[2]);
                        t.scale = Vec3::new(
                            descriptor.default_scale[0],
                            descriptor.default_scale[1],
                            descriptor.default_scale[2],
                        );
                    }
                    pending_selection = Some(Some(e));
                }
                SceneCommand::InstantiateDefinition { definition_ref, position, rotation_ypr, scale } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    log::debug!(
                        "definitions.runtime: command RuntimeCommand::InstantiateDefinition definition_ref='{}'",
                        definition_ref
                    );
                    let graph = AssetGraphResolver::resolve_root_ref(&definition_ref);
                    let transform = definitions_runtime::DefinitionInstantiateTransform {
                        translation: position,
                        rotation_ypr,
                        scale,
                    };
                    let (entity, trace) = definitions_runtime::apply_definition_instantiation(
                        world,
                        Some(root),
                        definition_ref,
                        transform,
                        graph,
                    );
                    log::debug!(
                        "definitions.runtime: trace definition_ref='{}' entity={:?} graph_nodes={} render_drawables={} materials={} textures={} physics_refs={} result='{}'",
                        trace.definition_ref,
                        entity,
                        trace.resolved_graph.nodes.len(),
                        trace.render_packet_request.drawable_refs.len(),
                        trace.render_packet_request.material_refs.len(),
                        trace.render_packet_request.texture_refs.len(),
                        trace.physics_declaration.physics_refs.len() + trace.physics_declaration.collision_refs.len(),
                        trace.apply_result
                    );
                    pending_selection = Some(Some(entity));
                }
                SceneCommand::SetTransform {
                    entity,
                    position,
                    rotation_ypr,
                    scale,
                } => {
                    let world = scene.world_mut();
                    if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
                        t.position = Vec3::new(position[0], position[1], position[2]);
                        t.rotation = Quat::from_euler(
                            EulerRot::YXZ,
                            rotation_ypr[0],
                            rotation_ypr[1],
                            rotation_ypr[2],
                        );
                        t.scale = Vec3::new(scale[0], scale[1], scale[2]);
                    }
                }
                SceneCommand::SetPrimitiveColor { entity, color } => {
                    let world = scene.world_mut();
                    if let Some(p) = world.get_mut_tracked::<Primitive>(entity) {
                        p.color = color;
                    }
                    let base = world
                        .get::<PrimitiveMaterialBase>(entity)
                        .map(|x| effective_material_base(x.id, default_mat))
                        .unwrap_or(default_mat);
                    ensure_primitive_base(world, entity, base);
                    apply_primitive_instance(world, &*mats, entity, base, color);
                }
                SceneCommand::SetMaterial { entity, material } => {
                    let world = scene.world_mut();
                    if world.get::<Primitive>(entity).is_some() {
                        let base = effective_material_base(material, default_mat);
                        let color = world
                            .get::<Primitive>(entity)
                            .map(|p| p.color)
                            .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                        ensure_primitive_base(world, entity, base);
                        apply_primitive_instance(world, &*mats, entity, base, color);
                    } else {
                        let _ = world.insert(entity, MaterialRef { id: material });
                    }
                }
                SceneCommand::UpdateMaterial { material, desc } => {
                    let _ = mats.set_desc(material, desc);
                }
                SceneCommand::SetAmbientLight { color, intensity } => {
                    let world = scene.world_mut();
                    match world.resource_mut::<AmbientLight>() {
                        Some(a) => {
                            a.color = color;
                            a.intensity = intensity;
                        }
                        None => world.insert_resource(AmbientLight { color, intensity }),
                    }
                }
                SceneCommand::SetDirectionalLight {
                    entity,
                    direction_ws,
                    color,
                    intensity,
                } => {
                    let world = scene.world_mut();
                    if let Some(dl) = world.get_mut_tracked::<DirectionalLight>(entity) {
                        dl.direction_ws = direction_ws;
                        dl.color = color;
                        dl.intensity = intensity;
                    }
                }
                SceneCommand::SetPointLight {
                    entity,
                    color,
                    intensity,
                    range,
                } => {
                    let world = scene.world_mut();
                    if let Some(pl) = world.get_mut_tracked::<PointLight>(entity) {
                        pl.color = color;
                        pl.intensity = intensity;
                        pl.range = range;
                    }
                }
                SceneCommand::SetPhysicsBody { entity, body } => {
                    let world = scene.world_mut();
                    ensure_physics_body(world, entity, body);
                }
                SceneCommand::ClearPhysicsBody { entity } => {
                    let world = scene.world_mut();
                    remove_physics_body(world, entity);
                    restore_non_collision_bounds(world, &prims, entity);
                }
                SceneCommand::SetDisplayVisibility { entity, mode } => {
                    let world = scene.world_mut();
                    let _ = world.insert(entity, DisplayVisibility { mode });
                }
                SceneCommand::SetParent { child, parent } => {
                    let world = scene.world_mut();
                    let _ = newengine_transform::set_parent(world, child, parent);
                    pending_selection = Some(Some(child));
                }
                SceneCommand::SetPlayMode { mode } => {
                    next_mode = Some(mode);
                }

            }
        }

        if let Some(mode) = next_mode {
            *self.play_mode.lock() = mode;
        }
        if let Some(sel) = pending_selection {
            self.set_selection(sel);
        }
    }
}
