fn spawn_equipped_weapon_visual(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    owner: EntityId,
    binding: EquippedWeaponBinding,
) -> Result<EntityId, String> {
    let definition = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(binding.item))
        .cloned()
        .ok_or_else(|| "equipped item definition is unavailable".to_owned())?;
    let world_definition = definition.world.clone().sanitized();
    let model_ref = world_definition.model_ref.as_deref().ok_or_else(|| {
        format!(
            "equipped weapon '{}' has no authored model",
            definition.name
        )
    })?;
    let avatar_root = world
        .get::<PlayerModelBinding>(owner)
        .and_then(|binding| binding.visual_root)
        .filter(|root| world.exists(*root))
        .ok_or_else(|| "player avatar visual root is not ready".to_owned())?;
    if definition.weapon_animation.skeleton.is_some() {
        let admission = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            spawn_skinned_equipped_weapon_visual(
                world,
                prims,
                mats,
                owner,
                binding,
                &definition,
                &world_definition,
                model_ref,
                avatar_root,
            )
        }));
        return match admission {
            Ok(result) => result,
            Err(payload) => {
                let panic_message = payload
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("<non-string panic payload>");
                eprintln!(
                    "GAME_READY_SKINNED_WEAPON_PANIC owner={} item='{}' model='{}': {}",
                    owner.stable_u64(),
                    definition.name,
                    model_ref,
                    panic_message,
                );
                newengine_ulog_api::ulog::error!(
                    "game-ready: skinned weapon admission panicked owner={} item='{}' model='{}': {}",
                    owner.stable_u64(),
                    definition.name,
                    model_ref,
                    panic_message,
                );
                Err(format!(
                    "equipped skinned weapon admission panicked: {panic_message}"
                ))
            }
        };
    }
    let decoded = decode_runtime_ydd_prefab(model_ref)
        .map_err(|error| format!("equipped weapon model decode failed '{model_ref}': {error}"))?;
    let alignment = weapon_visual_alignment(&decoded, definition.weapon_presentation.enabled)?;
    // Resolve every authored material before admitting the visual. A temporary materials-service
    // gap must defer the whole weapon instead of freezing one or more parts on diagnostic black.
    let material_ids = decoded
        .iter()
        .enumerate()
        .map(|(part_index, part)| {
            register_equipped_part_material(
                mats,
                &definition.name,
                part_index,
                part,
                world_definition.material_library_ref.as_deref(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let root = spawn_named(world, format!("Player/EquippedWeapon/{}", definition.name));
    let _ = world.insert(root, Transform::default());
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
    let mut spawned = 0usize;
    for (part_index, part) in decoded.iter().enumerate() {
        if !prims.is_registered(part.primitive_id) {
            prims.register_mesh(part.primitive_id, part.name.clone(), part.mesh.clone());
        }
        let material_id = material_ids[part_index];
        let entity = spawn_game_primitive(
            world,
            &*prims,
            mats,
            PrimitiveSpawnSpec {
                parent: root,
                primitive_id: part.primitive_id,
                material_id,
                name: &format!(
                    "Player/EquippedWeapon/{}/{}-{part_index}",
                    definition.name, part.material_slot
                ),
                // Translate the authored grip to the visual root. Translation must include
                // authored scale because local points are transformed as T * S * p.
                position: Vec3::new(
                    -alignment.grip_pivot.x * authored_scale.x,
                    -alignment.grip_pivot.y * authored_scale.y,
                    -alignment.grip_pivot.z * authored_scale.z,
                ),
                scale: authored_scale,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options: equipped_weapon_render_options(),
            },
        );
        let _ = world.insert(entity, EquippedWeaponVisualPart { owner, root });
        let _ = world.insert(
            entity,
            PlayerVisualPart {
                owner,
                part_index: part_index as u32,
                kind: PlayerVisualKind::EquippedWeapon,
                material_slot: part.material_slot.clone(),
            },
        );
        let _ = world.insert(
            entity,
            PlayerViewVisibility {
                base_mode: DisplayMode::GameOnly,
                policy: PlayerViewVisibilityPolicy::AlwaysVisible,
            },
        );
        spawned += 1;
    }
    if spawned == 0 {
        clear_equipped_weapon_visual(world, owner);
        return Err("equipped weapon model contains no renderable parts".to_owned());
    }

    newengine_ulog_api::ulog::info!(
        "game-ready: equipped weapon visual bound player={} item='{}' instance={} model='{}' parts={} attachment='readyhold-spined/bilateral-hand-ik' alignment='calibrated-palm-contacts'",
        owner.stable_u64(),
        definition.name,
        binding.instance_id.0,
        model_ref,
        spawned,
    );
    Ok(root)
}
