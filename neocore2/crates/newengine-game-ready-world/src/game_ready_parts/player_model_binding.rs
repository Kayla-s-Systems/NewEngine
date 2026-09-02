use super::*;

use super::animation::{prepare_player_animation_binding, PlayerAnimationRuntimeBinding};
use super::assets::ensure_player_runtime_model_parts;

fn assignment_from_spec(
    spec: &self::content::GameReadyPlayerModelSpec,
) -> newengine_engine_runtime::gameplay::PlayerModelAssignment {
    let animation = |slot: &str, legacy: &Option<String>| {
        spec.animation_slots
            .get(slot)
            .cloned()
            .or_else(|| legacy.clone())
    };
    newengine_engine_runtime::gameplay::PlayerModelAssignment {
        revision: 0,
        enabled: spec.enabled,
        source: spec.source.clone(),
        properties_ref: spec.properties_ref.clone(),
        texture_dictionary: spec.texture_dictionary.clone(),
        skeleton_source: spec.skeleton.clone(),
        animation_slots: spec.animation_slots.clone(),
        idle_animation: animation("idle", &spec.idle_animation),
        walk_animation: animation("walk", &spec.walk_animation),
        run_animation: animation("run", &spec.run_animation),
        sprint_animation: animation("sprint", &spec.sprint_animation),
        crouch_idle_animation: animation("crouch_idle", &spec.crouch_idle_animation),
        crouch_walk_animation: animation("crouch_walk", &spec.crouch_walk_animation),
        jump_animation: animation("jump", &spec.jump_animation),
        fall_animation: animation("fall", &spec.fall_animation),
        presentation: newengine_engine_runtime::gameplay::PlayerCharacterPresentation {
            animation_slots: spec.animation_slots.clone(),
            animation_event_bindings: spec.animation_event_bindings.clone(),
            detached_head_follow: spec.detached_head_follow,
            detached_head_follow_rule: spec.detached_head_follow_rule.clone(),
            eye_parent_follow: spec.eye_parent_follow,
            eye_parent_follow_rule: spec.eye_parent_follow_rule.clone(),
            helper_pose_copies: spec.helper_pose_copies.clone(),
            skin_sidecar: spec.skin_sidecar.clone(),
            braid_secondary_motion: spec.braid_secondary_motion.clone(),
            equipment_ready_animation: spec.equipment_ready_animation.clone(),
            equipment_aim_animation: spec.equipment_aim_animation.clone(),
            equipment_reload_animation: spec.equipment_reload_animation.clone(),
            unarmed_ready_animation: spec.unarmed_ready_animation.clone(),
            unarmed_attack_animation: spec.unarmed_attack_animation.clone(),
            turn_45_left_animation: spec.turn_45_left_animation.clone(),
            turn_45_right_animation: spec.turn_45_right_animation.clone(),
            turn_90_left_animation: spec.turn_90_left_animation.clone(),
            turn_90_right_animation: spec.turn_90_right_animation.clone(),
            turn_135_left_animation: spec.turn_135_left_animation.clone(),
            turn_135_right_animation: spec.turn_135_right_animation.clone(),
            turn_180_left_animation: spec.turn_180_left_animation.clone(),
            turn_180_right_animation: spec.turn_180_right_animation.clone(),
            fall_low_animation: spec.fall_low_animation.clone(),
            fall_medium_animation: spec.fall_medium_animation.clone(),
            fall_high_animation: spec.fall_high_animation.clone(),
            landing_soft_animation: spec.landing_soft_animation.clone(),
            landing_medium_animation: spec.landing_medium_animation.clone(),
            landing_hard_animation: spec.landing_hard_animation.clone(),
            landing_hard_run_animation: spec.landing_hard_run_animation.clone(),
            fall_medium_min_distance: spec.fall_medium_min_distance,
            fall_high_min_distance: spec.fall_high_min_distance,
            equipment_ready_sample_phase: spec.equipment_ready_sample_phase,
            equipment_ready_sample_phases: spec.equipment_ready_sample_phases.clone(),
            equipment_ready_rotation_weights: spec.equipment_ready_rotation_weights.clone(),
            equipment_aim_rotation_weights: spec.equipment_aim_rotation_weights.clone(),
            equipment_reload_rotation_weights: spec.equipment_reload_rotation_weights.clone(),
            equipment_arm_ik: spec.equipment_arm_ik,
            equipment_arm_ik_rig: spec.equipment_arm_ik_rig.clone(),
            ..newengine_engine_runtime::gameplay::PlayerCharacterPresentation::default()
        },
        target_height: spec.target_height,
        eye_height_ratio: spec.eye_height_ratio,
        local_offset: spec.local_offset,
        yaw_offset: spec.yaw_offset,
        hide_in_first_person: spec.hide_in_first_person,
    }
}

#[inline]
fn model_label(source: &str) -> String {
    let normalized = source.trim().replace('\\', "/");
    let raw = normalized
        .rsplit_once('@')
        .map(|(_, selector)| selector)
        .filter(|selector| !selector.trim().is_empty())
        .or_else(|| normalized.rsplit('/').next())
        .unwrap_or("model");
    let raw = raw
        .split('.')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("model");
    let label = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if label.is_empty() {
        "model".to_owned()
    } else {
        label
    }
}
fn set_player_fallback_visibility(
    world: &mut newengine_ecs::World,
    player: EntityId,
    mode: newengine_engine_runtime::gameplay::DisplayMode,
) {
    let visuals = world
        .query::<newengine_engine_runtime::gameplay::PlayerVisualPart>()
        .filter_map(|(entity, part)| {
            (part.owner == player
                && matches!(
                    part.kind,
                    newengine_engine_runtime::gameplay::PlayerVisualKind::FallbackCapsule
                ))
            .then_some(entity)
        })
        .collect::<Vec<_>>();

    for entity in visuals {
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::DisplayVisibility { mode },
        );
    }
}

pub(super) fn hide_player_fallback_visuals(world: &mut newengine_ecs::World, player: EntityId) {
    set_player_fallback_visibility(
        world,
        player,
        newengine_engine_runtime::gameplay::DisplayMode::RuntimeHidden,
    );
}

fn clear_player_runtime_model_visuals(world: &mut newengine_ecs::World, player: EntityId) {
    let parts = world
        .query::<newengine_engine_runtime::gameplay::PlayerVisualPart>()
        .filter_map(|(entity, part)| {
            (part.owner == player
                && matches!(
                    part.kind,
                    newengine_engine_runtime::gameplay::PlayerVisualKind::RuntimeModelPart
                ))
            .then_some(entity)
        })
        .collect::<Vec<_>>();
    for entity in parts {
        let _ = world.despawn(entity);
    }

    let visual_root = world
        .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
        .and_then(|binding| binding.visual_root);
    if let Some(visual_root) = visual_root.filter(|entity| world.exists(*entity)) {
        let _ = world.despawn(visual_root);
    }
}

fn mark_assignment_attempted(world: &mut newengine_ecs::World, player: EntityId, revision: u64) {
    if let Some(binding) =
        world.get_mut::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
    {
        binding.assignment_revision = revision;
    }
}

fn clear_player_model_binding(
    world: &mut newengine_ecs::World,
    player: EntityId,
    assignment_revision: u64,
) {
    clear_player_runtime_model_visuals(world, player);
    super::sidecar::clear_player_skin_sidecar(world, player);
    let _ = crate::player_hair::unbind_player_hair_v1(world, player);
    let _ = world.remove::<PlayerAnimationRuntimeBinding>(player);
    let _ = world
        .remove::<newengine_engine_runtime::gameplay::PlayerAuthoredAnimationCapabilities>(player);
    let _ = world.remove::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player);
    let _ = world.remove::<newengine_model_contact_api::ModelFootPoseState>(player);
    if let Some(binding) =
        world.get_mut::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
    {
        *binding = newengine_engine_runtime::gameplay::PlayerModelBinding {
            assignment_revision,
            ..Default::default()
        };
    }
    if world
        .get::<newengine_engine_runtime::gameplay::PlayerActor>(player)
        .is_some()
    {
        set_player_fallback_visibility(
            world,
            player,
            newengine_engine_runtime::gameplay::DisplayMode::GameOnly,
        );
    } else if world
        .get::<newengine_engine_runtime::gameplay::DisplayVisibility>(player)
        .is_some()
    {
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::DisplayVisibility {
                mode: newengine_engine_runtime::gameplay::DisplayMode::GameOnly,
            },
        );
    }
}

fn joint_is_descendant_of(
    skeleton: &newengine_model_skeleton_api::ModelSkeletonMetadata,
    mut joint_index: usize,
    ancestor_index: usize,
) -> bool {
    let mut guard = 0usize;
    loop {
        if joint_index == ancestor_index {
            return true;
        }
        if guard >= skeleton.joints.len() {
            return false;
        }
        let Some(parent) = skeleton
            .joints
            .get(joint_index)
            .and_then(|joint| joint.parent_index)
            .map(|index| index as usize)
            .filter(|index| *index < skeleton.joints.len())
        else {
            return false;
        };
        if parent == joint_index {
            return false;
        }
        joint_index = parent;
        guard += 1;
    }
}

#[inline]
fn part_is_first_person_near_body_semantic(part: &PlayerRuntimeModelPart) -> bool {
    let mesh = part.source_mesh_name.to_ascii_lowercase();
    let slot = part.material_slot.to_ascii_lowercase();
    // Semantic names are authoritative when available and also cover rigid face accessories that
    // have no skin stream. Keep the list deliberately anatomical/near-camera; ordinary torso/arm
    // meshes are not suppressed merely because they belong to a player. Clothing occluders such as
    // collars and hoods intentionally remain visible: hiding them exposes the very neck cavity FPP
    // presentation is required to cover.
    const TOKENS: &[&str] = &[
        "head", "face", "scalp", "hair", "eye", "eyeball", "lash", "brow", "teeth", "tooth",
        "tongue", "mouth", "oral", "gum", "neck",
    ];
    TOKENS
        .iter()
        .any(|token| mesh.contains(token) || slot.contains(token))
}

/// Full-body first person keeps the body/arms but suppresses camera-near shells. Semantic mesh or
/// material names win when authored; otherwise skin ownership provides a generic fallback for
/// head/neck pieces. This keeps the same world character and shadow caster while preventing
/// face/eyes/hair/neck geometry from surrounding the local camera.
fn runtime_part_visibility_policy(
    part: &PlayerRuntimeModelPart,
    skeleton: Option<&newengine_model_skeleton_api::ModelSkeletonMetadata>,
) -> newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy {
    const HEAD_OWNERSHIP_HIDE_RATIO: f32 = 0.45;
    const HEAD_PARENT_OWNERSHIP_HIDE_RATIO: f32 = 0.72;
    if part_is_first_person_near_body_semantic(part) {
        return newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::HideInFirstPerson;
    }
    let (Some(skeleton), Some(skin)) = (skeleton, part.skin.as_ref()) else {
        return newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible;
    };
    let Some(head_index) = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == skeleton.anchors.head)
    else {
        return newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible;
    };
    let root_index = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == skeleton.anchors.root);
    // Generic/non-humanoid metadata may legally collapse semantic anchors to root. Never let that
    // turn the whole skinned entity into an FPP-hidden "head".
    if root_index == Some(head_index) {
        return newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible;
    }

    let head_parent_index = skeleton
        .joints
        .get(head_index)
        .and_then(|joint| joint.parent_index)
        .map(|index| index as usize)
        .filter(|index| *index < skeleton.joints.len())
        .filter(|index| Some(*index) != root_index && *index != head_index);

    let mut total_weight = 0.0_f32;
    let mut head_weight = 0.0_f32;
    let mut head_parent_weight = 0.0_f32;
    for vertex in &skin.vertices {
        for (&joint, &weight) in vertex
            .joints
            .iter()
            .zip(vertex.weights.iter())
            .chain(vertex.joints_extra.iter().zip(vertex.weights_extra.iter()))
        {
            if !weight.is_finite() || weight <= 0.0 {
                continue;
            }
            total_weight += weight;
            let joint_index = usize::from(joint);
            if joint_index < skeleton.joints.len() {
                if joint_is_descendant_of(skeleton, joint_index, head_index) {
                    head_weight += weight;
                }
                if head_parent_index.is_some_and(|parent_index| {
                    joint_is_descendant_of(skeleton, joint_index, parent_index)
                }) {
                    head_parent_weight += weight;
                }
            }
        }
    }
    if total_weight > 1.0e-5
        && (head_weight / total_weight >= HEAD_OWNERSHIP_HIDE_RATIO
            || (head_parent_index.is_some()
                && head_parent_weight / total_weight >= HEAD_PARENT_OWNERSHIP_HIDE_RATIO))
    {
        newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::HideInFirstPerson
    } else {
        newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible
    }
}

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
                "game-ready: optional player hair preparation unavailable player={} definition={:?} err='{}' action='keep authored source hair meshes'",
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
            "game-ready: player skeletal animation set bound player={} clips='{}' skeleton_joints={} palette_joints={} supplemental_joints={} policy='semantic locomotion -> YCD -> local pose -> global -> inverse-bind -> model-space palette'",
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
                    "game-ready: player NEHAIR cutover committed player={} source_meshes_replaced={} policy='compiled groom active before source hair-card removal; native braid meshes remain independently owned'",
                    player.stable_u64(),
                    replaced,
                );
            }
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: optional player NEHAIR bind failed player={} err='{}' action='retain authored source hair meshes'",
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

#[inline]
fn player_capsule_ground_offset_y(world: &newengine_ecs::World, player: EntityId) -> f32 {
    if let Some(body) = world.get::<newengine_engine_runtime::gameplay::PhysicsBodyDesc>(player) {
        if let newengine_engine_runtime::gameplay::CollisionShapeDesc::Capsule {
            radius,
            half_height,
        } = body.shape.sanitized()
        {
            return -(half_height + radius);
        }
    }
    world
        .get::<newengine_engine_runtime::gameplay::CharacterBody>(player)
        .map(|body| {
            let body = body.sanitized();
            -(body.standing_half_height + body.radius)
        })
        .unwrap_or(0.0)
}

/// Keeps the authored avatar root anchored to the capsule sole while stance geometry changes.
///
/// `apply_player_stance_geometry` moves the capsule center when half-height changes so the
/// physics sole stays on the same support plane. A model root parented to that center must use
/// the *current* capsule extent as its inverse local offset; a standing-only offset makes the
/// whole avatar follow the crouched center below the floor.
pub(crate) fn tick_player_model_grounding(world: &mut newengine_ecs::World) {
    let players = world
        .query::<newengine_engine_runtime::gameplay::PlayerModelBinding>()
        .filter_map(|(player, binding)| binding.visual_root.map(|root| (player, root)))
        .collect::<Vec<_>>();

    for (player, visual_root) in players {
        if !world.exists(visual_root) {
            continue;
        }
        let local_offset = world
            .get::<newengine_engine_runtime::gameplay::PlayerModelAssignment>(player)
            .map(|assignment| assignment.local_offset)
            .unwrap_or(Vec3::ZERO);
        let grounded_local_y = local_offset.y + player_capsule_ground_offset_y(world, player);
        if let Some(transform) = world.get_mut::<Transform>(visual_root) {
            transform.position.x = local_offset.x;
            transform.position.y = grounded_local_y;
            transform.position.z = local_offset.z;
        }
    }
}

/// Applies runtime model assignment changes without replacing the PlayerActor.
/// Physics, inventory, input possession and camera targeting survive avatar swaps.
pub(crate) fn tick_player_model_assignments(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
) {
    let pending = world
        .query::<newengine_engine_runtime::gameplay::PlayerModelAssignment>()
        .filter_map(|(player, assignment)| {
            let bound_revision = world
                .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
                .map(|binding| binding.assignment_revision)
                .unwrap_or(0);
            (assignment.revision != bound_revision).then_some((player, assignment.clone()))
        })
        .collect::<Vec<_>>();

    for (player, assignment) in pending {
        let ground_offset = player_capsule_ground_offset_y(world, player);
        if let Err(error) =
            bind_player_model_assignment(world, prims, mats, player, &assignment, ground_offset)
        {
            // Record the attempted revision so a bad asset does not spam every frame. Assigning
            // another model increments the revision and immediately retries with the new source.
            mark_assignment_attempted(world, player, assignment.revision);
            newengine_ulog_api::ulog::warn!(
                "game-ready: player model assignment failed player={} revision={} source='{}': {}",
                player.stable_u64(),
                assignment.revision,
                assignment.source,
                error
            );
        }
    }
}
pub(crate) fn spawn_game_ready_player_model(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    player: EntityId,
    spec: &self::content::GameReadyPlayerModelSpec,
    capsule_ground_offset_y: f32,
) -> bool {
    let requested = assignment_from_spec(spec);
    let revision = match newengine_engine_runtime::gameplay::set_player_model_assignment(
        world, player, requested,
    ) {
        Ok(revision) => revision,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready: player model assignment rejected player={}: {}",
                player.stable_u64(),
                error
            );
            return false;
        }
    };
    let Some(assignment) = world
        .get::<newengine_engine_runtime::gameplay::PlayerModelAssignment>(player)
        .cloned()
    else {
        return false;
    };

    match bind_player_model_assignment(
        world,
        prims,
        mats,
        player,
        &assignment,
        capsule_ground_offset_y,
    ) {
        Ok(bound) => bound,
        Err(error) => {
            mark_assignment_attempted(world, player, revision);
            newengine_ulog_api::ulog::warn!(
                "game-ready: player model binding failed revision={} source='{}': {}",
                revision,
                assignment.source,
                error
            );
            false
        }
    }
}

#[cfg(test)]
mod grounding_tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{
        apply_player_stance_geometry, spawn_default_player, PlayerModelAssignment,
        PlayerModelBinding, PlayerStanceKind,
    };

    #[test]
    fn first_person_semantic_mask_hides_face_shell_without_skin_stream() {
        let face = PlayerRuntimeModelPart {
            source_mesh_name: "character_face_shell".to_owned(),
            primitive_id: PrimitiveId(1),
            first_person_primitive_id: None,
            material_id: MaterialId(1),
            material_slot: "m_face".to_owned(),
            color: [1.0; 4],
            skin: None,
        };
        let body = PlayerRuntimeModelPart {
            source_mesh_name: "character_torso".to_owned(),
            primitive_id: PrimitiveId(2),
            first_person_primitive_id: None,
            material_id: MaterialId(2),
            material_slot: "m_body".to_owned(),
            color: [1.0; 4],
            skin: None,
        };
        assert_eq!(
            runtime_part_visibility_policy(&face, None),
            newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::HideInFirstPerson
        );
        assert_eq!(
            runtime_part_visibility_policy(&body, None),
            newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible
        );
    }

    #[test]
    fn visual_root_preserves_world_foot_plane_when_crouching() {
        let mut world = newengine_ecs::World::new();
        let player = spawn_default_player(
            &mut world,
            None,
            "crouch-grounding",
            Vec3::new(2.0, 3.0, -4.0),
        );
        let visual_root = world.spawn();
        let local_offset = Vec3::new(0.15, 0.08, -0.12);
        let _ = world.insert(
            visual_root,
            Transform {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        set_parent(&mut world, visual_root, Some(player));
        let _ = world.insert(
            player,
            PlayerModelAssignment {
                enabled: true,
                revision: 1,
                local_offset,
                ..PlayerModelAssignment::default()
            },
        );
        let _ = world.insert(
            player,
            PlayerModelBinding {
                assignment_revision: 1,
                visual_root: Some(visual_root),
                ..PlayerModelBinding::default()
            },
        );

        tick_player_model_grounding(&mut world);
        let standing_center_y = world
            .get::<Transform>(player)
            .expect("player transform")
            .position
            .y;
        let standing_root_y = world
            .get::<Transform>(visual_root)
            .expect("visual transform")
            .position
            .y;
        let standing_world_anchor_y = standing_center_y + standing_root_y;

        assert!(
            apply_player_stance_geometry(&mut world, player, PlayerStanceKind::Crouched, 41),
            "crouch geometry must apply"
        );
        tick_player_model_grounding(&mut world);

        let crouched_center_y = world
            .get::<Transform>(player)
            .expect("player transform")
            .position
            .y;
        let crouched_root_y = world
            .get::<Transform>(visual_root)
            .expect("visual transform")
            .position
            .y;
        let crouched_world_anchor_y = crouched_center_y + crouched_root_y;

        assert!(
            (standing_world_anchor_y - crouched_world_anchor_y).abs() <= 1.0e-5,
            "visual root moved through support plane standing={standing_world_anchor_y} crouched={crouched_world_anchor_y}"
        );
        assert!(
            crouched_root_y > standing_root_y,
            "shorter crouch capsule must raise child local root to compensate the lowered capsule center"
        );
        assert!(
            (world.get::<Transform>(visual_root).unwrap().position.x - local_offset.x).abs()
                <= 1.0e-6
        );
        assert!(
            (world.get::<Transform>(visual_root).unwrap().position.z - local_offset.z).abs()
                <= 1.0e-6
        );
    }
}
