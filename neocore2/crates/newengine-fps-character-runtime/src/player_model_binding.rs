use super::*;

use super::animation::{prepare_player_animation_binding, PlayerAnimationRuntimeBinding};
use super::assets::ensure_player_runtime_model_parts;

include!("player_model_binding/helpers.rs");

fn bind_player_model_assignment(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    player: EntityId,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    capsule_ground_offset_y: f32,
) -> Result<bool, String> {
    let possessed_player = world
        .get::<newengine_engine_runtime::gameplay::PlayerActor>(player)
        .is_some();
    if !assignment.enabled || assignment.source.trim().is_empty() {
        clear_player_model_binding(world, player, assignment.revision);
        return Ok(false);
    }

    super::validation::validate_player_asset_family(assignment)?;

    // Resolve/register first. A bad replacement assignment must not destroy the currently
    // visible avatar; the presentation swap happens only after the replacement is ready.
    let (model_source, parts, skeleton) =
        ensure_player_runtime_model_parts(prims, mats, assignment)?;
    super::validation::validate_player_skin_contract(assignment, &parts, skeleton.as_ref())?;
    let animation_binding = prepare_player_animation_binding(
        assignment,
        &parts,
        skeleton.as_ref(),
    )
    .map_err(|error| {
        format!(
            "required playable-character skeletal animation binding failed player={} source={} err={} policy=no_bind_or_default_pose_fallback",
            player.stable_u64(),
            assignment.source,
            error
        )
    })?;
    if animation_binding.is_none() && parts.iter().any(|part| part.skin.is_some()) {
        return Err(format!(
            "skinned playable character has no authored animation binding player={} source={} policy=authored_animation_required",
            player.stable_u64(),
            assignment.source,
        ));
    }

    let prepared_sidecar = super::sidecar::prepare_player_skin_sidecar(
        prims,
        mats,
        assignment,
        &parts,
        skeleton.as_ref(),
    )?;

    let mut prepared_hair = match crate::player_hair::prepare_player_hair_from_assignment_v1(
        player,
        assignment,
        skeleton.as_ref(),
    ) {
        Ok(binding) => binding,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "fps-character: optional player hair preparation unavailable player={} definition={:?} err='{}' action='keep authored source hair meshes'",
                player.stable_u64(),
                assignment.properties_ref,
                error
            );
            None
        }
    };
    if let Some(hair) = prepared_hair.as_mut() {
        hair.hide_in_first_person = possessed_player;
    }

    clear_player_runtime_model_visuals(world, player);
    super::sidecar::clear_player_skin_sidecar(world, player);
    let _ = crate::player_hair::unbind_player_hair_v1(world, player);
    let _ = world.remove::<PlayerAnimationRuntimeBinding>(player);
    let _ = world
        .remove::<newengine_engine_runtime::gameplay::PlayerAuthoredAnimationCapabilities>(player);
    let _ = world.remove::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player);
    let _ = world.remove::<newengine_model_contact_api::ModelFootPoseState>(player);

    let label = model_label(&model_source);
    let visual_root_name = format!("Player/Avatar/{label}");
    let visual_root = spawn_named(world, visual_root_name.clone());
    let visual_position = assignment.local_offset + Vec3::new(0.0, capsule_ground_offset_y, 0.0);
    let _ = world.insert(
        visual_root,
        Transform {
            position: visual_position,
            rotation: Quat::from_euler(EulerRot::YXZ, assignment.yaw_offset, 0.0, 0.0),
            scale: Vec3::ONE,
        },
    );
    newengine_engine_runtime::gameplay::attach_scene_object_core(
        world,
        visual_root,
        visual_position,
        Vec3::new(0.5, (assignment.target_height * 0.5).max(0.5), 0.5),
    );
    let _ = world.insert(
        visual_root,
        newengine_engine_runtime::gameplay::GameplayActor,
    );
    let _ = set_parent(world, visual_root, Some(player));

    // Character remains one world-space skinned entity in every gameplay camera mode. First person
    // keeps torso, arms, hands, legs and equipment visible; only camera-near head/face/neck shells
    // are suppressed or replaced by sealed FPP topology. Hair source meshes stay live until replacement binds.
    let first_person_active = possessed_player
        && world
            .resource::<newengine_engine_runtime::gameplay::PlayerViewState>()
            .copied()
            .unwrap_or_default()
            .first_person_active;
    let mut hair_source_entities = Vec::new();
    for (part_index, part) in parts.iter().enumerate() {
        let visibility_policy = if possessed_player {
            runtime_part_visibility_policy(part, skeleton.as_ref())
        } else {
            newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible
        };
        let entity = spawn_named(
            world,
            format!("{visual_root_name}/Part{part_index}:{}", part.material_slot),
        );
        if prepared_hair.as_ref().is_some_and(|hair| {
            crate::player_hair::source_mesh_replaced_by_hair_v1(hair, &part.source_mesh_name)
        }) {
            hair_source_entities.push(entity);
        }
        let _ = world.insert(entity, Transform::default());
        let initial_primitive_id = if first_person_active {
            part.first_person_primitive_id.unwrap_or(part.primitive_id)
        } else {
            part.primitive_id
        };
        let _ = world.insert(
            entity,
            Primitive {
                id: initial_primitive_id,
                color: part.color,
            },
        );
        if let Some(first_person_primitive) = part.first_person_primitive_id {
            let _ = world.insert(
                entity,
                newengine_engine_runtime::gameplay::PlayerFirstPersonPrimitiveVariant {
                    world_primitive: part.primitive_id,
                    first_person_primitive,
                },
            );
        }
        if let Some(bounds) = primitive_bounds(prims, part.primitive_id) {
            let _ = world.insert(entity, bounds);
        }
        newengine_engine_runtime::gameplay::attach_scene_object_core(
            world,
            entity,
            Vec3::ZERO,
            Vec3::splat(0.25),
        );
        let _ = world.insert(entity, newengine_engine_runtime::gameplay::GameplayActor);
        if let Some(skin) = part.skin.as_ref() {
            let _ = world.insert(
                entity,
                newengine_engine_runtime::gameplay::PlayerSkinBinding {
                    owner: player,
                    vertices: skin
                        .vertices
                        .iter()
                        .map(
                            |vertex| newengine_engine_runtime::gameplay::PlayerSkinVertex {
                                joints: vertex.joints,
                                weights: vertex.weights,
                                joints_extra: vertex.joints_extra,
                                weights_extra: vertex.weights_extra,
                            },
                        )
                        .collect(),
                    source_to_model: skin.source_to_model,
                },
            );
        }
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PlayerVisualPart {
                owner: player,
                part_index: part_index as u32,
                kind: newengine_engine_runtime::gameplay::PlayerVisualKind::RuntimeModelPart,
                material_slot: part.material_slot.clone(),
            },
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PlayerViewVisibility {
                base_mode: newengine_engine_runtime::gameplay::DisplayMode::GameOnly,
                policy: visibility_policy,
            },
        );
        let initial_mode = if first_person_active
            && visibility_policy
                == newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::HideInFirstPerson
        {
            newengine_engine_runtime::gameplay::DisplayMode::RuntimeHidden
        } else {
            newengine_engine_runtime::gameplay::DisplayMode::GameOnly
        };
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::DisplayVisibility { mode: initial_mode },
        );
        let _ = set_parent(world, entity, Some(visual_root));
        let _ = apply_exact_material(
            world,
            mats,
            entity,
            part.material_id,
            part.material_id,
            part.color,
        );
    }

    let sidecar_part_count = if let Some(prepared_sidecar) = prepared_sidecar {
        super::sidecar::bind_prepared_player_skin_sidecar(
            world,
            prims,
            mats,
            player,
            visual_root,
            &visual_root_name,
            parts.len(),
            first_person_active,
            false,
            prepared_sidecar,
        )?
    } else {
        0
    };

    if let Some(mut animation_binding) = animation_binding {
        let retained_animation_states =
            newengine_engine_runtime::gameplay::retained_animation_states(world, player);
        animation_binding
            .seed_semantic_state(&retained_animation_states)
            .map_err(|error| format!("seed player animation semantic state: {error}"))?;
        // One-shot landing presentation begins from the current gameplay revision. A model
        // rebind/hot-reload must not replay a historical landing as if it happened this frame.
        animation_binding.consume_landing_revision_baseline(
            world
                .get::<newengine_engine_runtime::gameplay::PlayerLandingState>(player)
                .map(|state| state.revision)
                .unwrap_or(0),
        );
        let initial_palette = animation_binding.initial_palette();
        let clip_refs = animation_binding.clip_refs_csv();
        let skeleton_joint_count = animation_binding.skeleton_joint_count();
        let supplemental_joint_count = animation_binding.supplemental_palette_joint_count();
        let joint_count = animation_binding.expected_palette_joints();
        super::validation::validate_player_palette(
            &initial_palette,
            joint_count,
            "initial animated palette",
        )?;
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::PlayerSkinPose {
                palette: initial_palette,
                revision: 1,
            },
        );
        let capabilities = animation_binding.authored_capabilities();
        let _ = world.insert(player, capabilities);
        let _ = world.insert(player, animation_binding);
        newengine_ulog_api::ulog::info!(
            "fps-character: player skeletal animation set bound player={} clips='{}' skeleton_joints={} palette_joints={} supplemental_joints={} policy='semantic locomotion -> YCD -> local pose -> global -> inverse-bind -> model-space palette'",
            player.stable_u64(),
            clip_refs,
            skeleton_joint_count,
            joint_count,
            supplemental_joint_count,
        );
    }

    if let Some(prepared_hair) = prepared_hair {
        match crate::player_hair::bind_prepared_player_hair_v1(world, player, prepared_hair) {
            Ok(()) => {
                let replaced = hair_source_entities.len();
                for entity in hair_source_entities {
                    if world.exists(entity) {
                        let _ = world.despawn(entity);
                    }
                }
                newengine_ulog_api::ulog::info!(
                    "fps-character: player NEHAIR cutover committed player={} source_meshes_replaced={} policy='compiled groom active before source hair-card removal; native braid meshes remain independently owned'",
                    player.stable_u64(),
                    replaced,
                );
            }
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "fps-character: optional player NEHAIR bind failed player={} err='{}' action='retain authored source hair meshes'",
                    player.stable_u64(),
                    error
                );
            }
        }
    }

    if let Some(binding) =
        world.get_mut::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
    {
        binding.assignment_revision = assignment.revision;
        binding.source = model_source.clone();
        binding.skeleton_source = skeleton.as_ref().map(|metadata| metadata.source.clone());
        binding.visual_root = Some(visual_root);
        binding.part_count = parts.len() as u32 + sidecar_part_count;
        binding.target_height = assignment.target_height;
        binding.feet_to_eye_height = skeleton
            .as_ref()
            .map(|metadata| metadata.anchors.eye_height)
            .unwrap_or(assignment.target_height * assignment.eye_height_ratio);
    }

    if possessed_player {
        hide_player_fallback_visuals(world, player);
        newengine_engine_runtime::gameplay::emit_player_event(
            world,
            player,
            newengine_engine_runtime::gameplay::PlayerEventKind::ModelBound,
            format!(
                "revision={} model='{}' skeleton='{}' parts={}",
                assignment.revision,
                model_source,
                skeleton
                    .as_ref()
                    .map(|metadata| metadata.source.as_str())
                    .unwrap_or("none"),
                parts.len() as u32 + sidecar_part_count
            ),
        );
    } else if world
        .get::<newengine_engine_runtime::gameplay::DisplayVisibility>(player)
        .is_some()
    {
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::DisplayVisibility {
                mode: newengine_engine_runtime::gameplay::DisplayMode::RuntimeHidden,
            },
        );
    }
    Ok(true)
}

include!("player_model_binding/runtime.rs");
include!("player_model_binding/tests.rs");
