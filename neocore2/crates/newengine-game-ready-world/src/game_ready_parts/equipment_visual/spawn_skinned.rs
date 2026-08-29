fn spawn_skinned_equipped_weapon_visual(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    owner: EntityId,
    binding: EquippedWeaponBinding,
    definition: &newengine_engine_runtime::gameplay::ItemDefinition,
    world_definition: &newengine_engine_runtime::gameplay::WorldItemDefinition,
    model_ref: &str,
    avatar_root: EntityId,
) -> Result<EntityId, String> {
    let skeleton_ref = definition
        .weapon_animation
        .skeleton
        .as_deref()
        .ok_or("skinned rifle has no authored skeleton reference")?;
    let request = newengine_model_domain_api::ModelAssetRequest::new(model_ref.to_owned())
        .with_skeleton(skeleton_ref.to_owned());
    let constructor =
        newengine_model_client::ModelGatewayClient::new(newengine_plugin_host::default_host_api());
    let bundle = constructor.assemble_bundle(&request).map_err(|error| {
        format!("equipped skinned rifle bundle failed model='{model_ref}': {error}")
    })?;
    let skeleton = bundle
        .skeleton
        .ok_or_else(|| format!("equipped skinned rifle has no skeleton model='{model_ref}'"))?;
    if bundle.parts.is_empty() {
        return Err(format!(
            "equipped skinned rifle contains no parts model='{model_ref}'"
        ));
    }

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut source_to_model = None;
    for part in &bundle.parts {
        for vertex in &part.mesh.vertices {
            let point = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            min = min.min(point);
            max = max.max(point);
        }
        let skin = part.skin.as_ref().ok_or_else(|| {
            format!(
                "equipped rifle native part '{}' has no skin stream",
                part.material_slot
            )
        })?;
        if skin.vertices.len() != part.mesh.vertices.len() {
            return Err(format!(
                "equipped rifle skin/mesh vertex mismatch slot='{}' skin={} vertices={}",
                part.material_slot,
                skin.vertices.len(),
                part.mesh.vertices.len()
            ));
        }
        match source_to_model {
            Some(existing) if existing != skin.source_to_model => {
                return Err("equipped rifle parts disagree on skin_source_to_model".to_owned());
            }
            None => source_to_model = Some(skin.source_to_model),
            _ => {}
        }
    }
    validate_canonical_rifle_visual_space(min, max)?;
    let source_to_model = source_to_model.ok_or("equipped rifle has no skin source_to_model")?;

    let root = spawn_named(world, format!("Player/EquippedWeapon/{}", definition.name));
    let initial_transform = Transform::default();
    let _ = world.insert(root, initial_transform);
    let _ = world.insert(root, newengine_transform::TransformEditRoot);
    let _ = world.insert(
        root,
        newengine_transform::RuntimeTransformEditOverride::new(initial_transform),
    );
    let last_shot_sequence = world
        .get::<PlayerWeaponState>(owner)
        .map(|state| state.shot_sequence)
        .unwrap_or(0);
    let _ = world.insert(
        root,
        EquippedWeaponVisualRoot {
            owner,
            instance_id: binding.instance_id,
            item: binding.item,
            grip_debug_emitted: false,
            aim_alpha: 0.0,
            last_shot_sequence,
            recoil_alpha: 0.0,
            recoil_yaw_radians: 0.0,
        },
    );
    let _ = world.insert(root, WeaponSecondaryDynamicsState::default());
    let _ = world.insert(
        root,
        DisplayVisibility {
            mode: DisplayMode::GameOnly,
        },
    );
    let _ = set_parent(world, root, Some(avatar_root));

    let authored_scale = Vec3::new(
        world_definition.scale[0],
        world_definition.scale[1],
        world_definition.scale[2],
    );
    for (part_index, part) in bundle.parts.into_iter().enumerate() {
        let primitive_id = PrimitiveId(fnv1a_64(&format!(
            "equipped-skinned:{}:revision={}:part={}:slot={}",
            bundle.source, binding.instance_id.0, part_index, part.material_slot
        )));
        if !prims.is_registered(primitive_id) {
            prims.register_mesh(
                primitive_id,
                format!(
                    "EquippedWeapon/Skinned/{}:{}",
                    definition.name, part.material_slot
                ),
                part.mesh,
            );
        }
        let material_name = part.material.material_ref.clone().unwrap_or_else(|| {
            format!("EquippedWeapon/{}/{}", definition.name, part.material_slot)
        });
        let material_id = mats.upsert_named_with_textures(
            &material_name,
            part.material.descriptor,
            part.material.textures.clone().sanitized(),
        );
        let entity = spawn_game_primitive(
            world,
            &*prims,
            mats,
            PrimitiveSpawnSpec {
                parent: root,
                primitive_id,
                material_id,
                name: &format!(
                    "Player/EquippedWeapon/{}/{}-{part_index}",
                    definition.name, part.material_slot
                ),
                position: Vec3::ZERO,
                scale: authored_scale,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options: equipped_weapon_render_options(),
            },
        );
        let skin = part.skin.expect("skin was validated above");
        let _ = world.insert(
            entity,
            PlayerSkinBinding {
                owner: root,
                vertices: skin
                    .vertices
                    .into_iter()
                    .map(|vertex| PlayerSkinVertex {
                        joints: vertex.joints,
                        weights: vertex.weights,
                        joints_extra: vertex.joints_extra,
                        weights_extra: vertex.weights_extra,
                    })
                    .collect(),
                source_to_model: skin.source_to_model,
            },
        );
        let _ = world.insert(
            entity,
            DisplayVisibility {
                mode: DisplayMode::RuntimeHidden,
            },
        );
        let _ = world.insert(entity, EquippedWeaponVisualPart { owner, root });
        let _ = world.insert(
            entity,
            PlayerVisualPart {
                owner,
                part_index: part_index as u32,
                kind: PlayerVisualKind::EquippedWeapon,
                material_slot: part.material_slot,
            },
        );
        let _ = world.insert(
            entity,
            PlayerViewVisibility {
                base_mode: DisplayMode::GameOnly,
                policy: PlayerViewVisibilityPolicy::AlwaysVisible,
            },
        );
    }

    if let Err(error) = crate::weapon_animation::bind_equipped_weapon_animation(
        world,
        root,
        owner,
        binding.instance_id,
        skeleton,
        source_to_model,
        &definition.weapon_animation,
        last_shot_sequence,
    ) {
        clear_equipped_weapon_visual(world, owner);
        return Err(format!(
            "equipped weapon animation admission failed: {error}"
        ));
    }

    // Skinned weapon parts stay quarantined while skeleton/YCD admission is incomplete. Once the
    // binding succeeds, reveal the whole weapon atomically. Previously these entities remained
    // RuntimeHidden forever, so the rifle could be fully simulated/animated while rendering no
    // geometry at all.
    let admitted_parts = world
        .query::<EquippedWeaponVisualPart>()
        .filter_map(|(entity, part)| (part.root == root).then_some(entity))
        .collect::<Vec<_>>();
    for entity in admitted_parts {
        let _ = world.insert(
            entity,
            DisplayVisibility {
                mode: DisplayMode::GameOnly,
            },
        );
    }

    newengine_ulog_api::ulog::info!(
        "game-ready: equipped weapon native skin bound player={} item='{}' model='{}' joints={} policy='authored weapon skin + YCD palette; character and weapon reload share PlayerWeaponState'",
        owner.stable_u64(),
        definition.name,
        model_ref,
        world.get::<newengine_engine_runtime::gameplay::PlayerSkinPose>(root)
            .map(|pose| pose.palette.len())
            .unwrap_or(0),
    );
    Ok(root)
}
