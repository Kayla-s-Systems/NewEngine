fn assignment_from_spec(
    spec: &crate::AuthoredPlayerModelSpec,
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
            skeletal_secondary_motion: spec.skeletal_secondary_motion.clone(),
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
