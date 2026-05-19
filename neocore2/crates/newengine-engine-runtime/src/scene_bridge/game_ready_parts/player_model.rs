use newengine_model_adapter::{ModelAssetAdapter, ModelAssetRequest, ModelSkeletonMetadata};

#[derive(Clone, Debug)]
struct PlayerRuntimeModelPart {
    primitive_id: PrimitiveId,
    material_id: MaterialId,
    material_slot: String,
    color: [f32; 4],
}

fn ensure_player_runtime_model_parts(
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    spec: &self::content::GameReadyPlayerModelSpec,
) -> Result<(String, Vec<PlayerRuntimeModelPart>, Option<ModelSkeletonMetadata>), String> {
    let mut request = ModelAssetRequest::new(spec.source.clone()).with_human_scale(spec.target_height, spec.eye_height_ratio);
    if let Some(dictionary) = spec.texture_dictionary.as_deref() {
        request = request.with_texture_dictionary(dictionary);
    }
    if let Some(skeleton) = spec.skeleton.as_deref() {
        request = request.with_skeleton(skeleton);
    }

    let adapter = ModelAssetAdapter::new();
    let bundle = adapter.load_bundle(&request)?;

    if let Some(metadata) = bundle.skeleton.as_ref() {
        log::info!(
            "game-ready: player skeleton metadata bound source='{}' skeleton='{}' format='{}' bytes={} joints={} status='{}'",
            bundle.source,
            metadata.source,
            metadata.source_format,
            metadata.byte_len,
            metadata.joints.len(),
            metadata.decode_status
        );
    }

    let mut out = Vec::with_capacity(bundle.parts.len());
    for part in bundle.parts {
        let primitive_id = PrimitiveId(fnv1a_64(&format!("player-model:{}:{}", bundle.source, part.material_slot)));
        if !prims.is_registered(primitive_id) {
            let vertex_count = part.mesh.vertices.len();
            let index_count = part.mesh.indices.len();
            prims.register_mesh(
                primitive_id,
                format!("PlayerModel/{} ({})", part.material_slot, bundle.source),
                part.mesh,
            );
            log::info!(
                "game-ready: player model part registered source='{}' material='{}' vertices={} indices={}",
                bundle.source,
                part.material_slot,
                vertex_count,
                index_count
            );
        }

        let material_id = mats.upsert_named_with_textures(
            &format!("Player/Abigail/{}", part.material_slot),
            part.material.descriptor,
            part.material.textures,
        );
        out.push(PlayerRuntimeModelPart {
            primitive_id,
            material_id,
            material_slot: part.material_slot,
            color: part.material.fallback_color,
        });
    }

    if let Some(dictionary) = bundle.texture_dictionary.as_deref() {
        log::info!(
            "game-ready: player model texture dictionary bound source='{}' dictionary='{}' materials={}",
            bundle.source,
            dictionary,
            out.len()
        );
    }

    if !bundle.collisions.is_empty() {
        log::info!(
            "game-ready: player model collision bindings derived source='{}' collisions={}",
            bundle.source,
            bundle.collisions.len()
        );
    }

    Ok((bundle.source, out, bundle.skeleton))
}

fn hide_player_fallback_visuals(world: &mut newengine_ecs::World, player: EntityId) {
    let hidden = world
        .query::<crate::gameplay::PlayerVisualPart>()
        .filter_map(|(entity, part)| {
            (part.owner == player && matches!(part.kind, crate::gameplay::PlayerVisualKind::FallbackCapsule)).then_some(entity)
        })
        .collect::<Vec<_>>();

    for entity in hidden {
        let _ = world.insert(entity, crate::gameplay::DisplayVisibility { mode: crate::gameplay::DisplayMode::RuntimeHidden });
    }
}

fn spawn_game_ready_player_model(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    player: EntityId,
    spec: &self::content::GameReadyPlayerModelSpec,
    capsule_ground_offset_y: f32,
) -> bool {
    if !spec.enabled || spec.source.trim().is_empty() {
        return false;
    }

    let (model_source, parts, skeleton) = match ensure_player_runtime_model_parts(prims, mats, spec) {
        Ok(model) => model,
        Err(e) => {
            log::warn!("game-ready: player model binding failed: {}", e);
            return false;
        }
    };

    let visual_root = spawn_named(world, "Player/Avatar/Abigail");
    let _ = world.insert(
        visual_root,
        Transform {
            position: spec.local_offset + Vec3::new(0.0, capsule_ground_offset_y, 0.0),
            rotation: Quat::from_euler(EulerRot::YXZ, spec.yaw_offset, 0.0, 0.0),
            scale: Vec3::ONE,
        },
    );
    let _ = world.insert(visual_root, crate::gameplay::GameplayActor);
    let _ = set_parent(world, visual_root, Some(player));

    let visibility_policy = if spec.hide_in_first_person {
        crate::gameplay::PlayerViewVisibilityPolicy::HideInFirstPerson
    } else {
        crate::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible
    };

    for (part_index, part) in parts.iter().enumerate() {
        let entity = spawn_named(world, format!("Player/Avatar/Abigail/Part{}", part_index));
        let _ = world.insert(entity, Transform::default());
        let _ = world.insert(entity, Primitive { id: part.primitive_id, color: part.color });
        let _ = world.insert(entity, crate::gameplay::GameplayActor);
        let _ = world.insert(
            entity,
            crate::gameplay::PlayerVisualPart {
                owner: player,
                part_index: part_index as u32,
                kind: crate::gameplay::PlayerVisualKind::RuntimeModelPart,
                material_slot: part.material_slot.clone(),
            },
        );
        let _ = world.insert(
            entity,
            crate::gameplay::PlayerViewVisibility {
                base_mode: crate::gameplay::DisplayMode::GameOnly,
                policy: visibility_policy,
            },
        );
        let initial_mode = if spec.hide_in_first_person {
            crate::gameplay::DisplayMode::RuntimeHidden
        } else {
            crate::gameplay::DisplayMode::GameOnly
        };
        let _ = world.insert(entity, crate::gameplay::DisplayVisibility { mode: initial_mode });
        let _ = set_parent(world, entity, Some(visual_root));
        let _ = apply_exact_material(world, mats, entity, part.material_id, part.material_id, part.color);
    }

    if let Some(binding) = world.get_mut::<crate::gameplay::PlayerModelBinding>(player) {
        binding.source = model_source.clone();
        binding.skeleton_source = skeleton.as_ref().map(|metadata| metadata.source.clone());
        binding.visual_root = Some(visual_root);
        binding.part_count = parts.len() as u32;
        binding.target_height = spec.target_height;
        binding.feet_to_eye_height = skeleton
            .as_ref()
            .map(|metadata| metadata.anchors.eye_height)
            .unwrap_or(spec.target_height * spec.eye_height_ratio);
    }

    hide_player_fallback_visuals(world, player);
    crate::gameplay::emit_player_event(
        world,
        player,
        crate::gameplay::PlayerEventKind::ModelBound,
        format!(
            "model='{}' skeleton='{}' parts={}",
            model_source,
            skeleton.as_ref().map(|metadata| metadata.source.as_str()).unwrap_or("none"),
            parts.len()
        ),
    );
    true
}
