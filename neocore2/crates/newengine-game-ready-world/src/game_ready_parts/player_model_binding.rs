use super::*;

use super::animation::{prepare_player_animation_binding, PlayerAnimationRuntimeBinding};
use super::assets::ensure_player_runtime_model_parts;

fn assignment_from_spec(
    spec: &self::content::GameReadyPlayerModelSpec,
) -> newengine_engine_runtime::gameplay::PlayerModelAssignment {
    newengine_engine_runtime::gameplay::PlayerModelAssignment {
        revision: 0,
        enabled: spec.enabled,
        source: spec.source.clone(),
        properties_ref: spec.properties_ref.clone(),
        texture_dictionary: spec.texture_dictionary.clone(),
        skeleton_source: spec.skeleton.clone(),
        idle_animation: spec.idle_animation.clone(),
        walk_animation: spec.walk_animation.clone(),
        run_animation: spec.run_animation.clone(),
        sprint_animation: spec.sprint_animation.clone(),
        crouch_idle_animation: spec.crouch_idle_animation.clone(),
        crouch_walk_animation: spec.crouch_walk_animation.clone(),
        jump_animation: spec.jump_animation.clone(),
        fall_animation: spec.fall_animation.clone(),
        presentation: newengine_engine_runtime::gameplay::PlayerCharacterPresentation {
            equipment_ready_animation: spec.equipment_ready_animation.clone(),
            equipment_aim_animation: spec.equipment_aim_animation.clone(),
            equipment_reload_animation: spec.equipment_reload_animation.clone(),
            unarmed_ready_animation: spec.unarmed_ready_animation.clone(),
            unarmed_attack_animation: spec.unarmed_attack_animation.clone(),
            equipment_ready_sample_phase: spec.equipment_ready_sample_phase,
            equipment_ready_rotation_weights: spec.equipment_ready_rotation_weights.clone(),
            equipment_aim_rotation_weights: spec.equipment_aim_rotation_weights.clone(),
            equipment_reload_rotation_weights: spec.equipment_reload_rotation_weights.clone(),
            equipment_arm_ik: spec.equipment_arm_ik,
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
    let _ = world.remove::<PlayerAnimationRuntimeBinding>(player);
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
    set_player_fallback_visibility(
        world,
        player,
        newengine_engine_runtime::gameplay::DisplayMode::GameOnly,
    );
}

fn bind_player_model_assignment(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    player: EntityId,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    capsule_ground_offset_y: f32,
) -> Result<bool, String> {
    if !assignment.enabled || assignment.source.trim().is_empty() {
        clear_player_model_binding(world, player, assignment.revision);
        return Ok(false);
    }

    super::validation::validate_player_asset_family(assignment)?;

    // Resolve/register first. A bad replacement assignment must not destroy the currently
    // visible avatar; the presentation swap happens only after the replacement is ready.
    let (model_source, parts, skeleton) =
        ensure_player_runtime_model_parts(prims, mats, assignment)?;
    let validated_skin_source_to_model =
        super::validation::validate_player_skin_contract(assignment, &parts, skeleton.as_ref())?;
    let animation_binding =
        prepare_player_animation_binding(assignment, &parts, skeleton.as_ref())?;

    clear_player_runtime_model_visuals(world, player);
    let _ = world.remove::<PlayerAnimationRuntimeBinding>(player);
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

    let visibility_policy = if assignment.hide_in_first_person {
        newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::HideInFirstPerson
    } else {
        newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible
    };

    for (part_index, part) in parts.iter().enumerate() {
        let entity = spawn_named(
            world,
            format!("{visual_root_name}/Part{part_index}:{}", part.material_slot),
        );
        let _ = world.insert(entity, Transform::default());
        let _ = world.insert(
            entity,
            Primitive {
                id: part.primitive_id,
                color: part.color,
            },
        );
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
        let initial_mode = if assignment.hide_in_first_person {
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

    if let Some(animation_binding) = animation_binding {
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
        let _ = world.insert(player, animation_binding);
        newengine_ulog_api::ulog::info!(
            "game-ready: player skeletal animation set bound player={} clips='{}' skeleton_joints={} palette_joints={} supplemental_joints={} policy='semantic locomotion -> YCD -> local pose -> global -> inverse-bind -> model-space palette'",
            player.stable_u64(),
            clip_refs,
            skeleton_joint_count,
            joint_count,
            supplemental_joint_count,
        );
    } else if parts.iter().any(|part| part.skin.is_some()) {
        let skeleton = skeleton
            .as_ref()
            .ok_or_else(|| "skinned player model requires authored skeleton metadata".to_owned())?;
        let joint_count = skeleton.joints.len();
        let source_to_model = validated_skin_source_to_model.ok_or_else(|| {
            "skinned player model has no validated source-to-model transform".to_owned()
        })?;
        let mut bind_palette = Vec::with_capacity(joint_count);
        newengine_animation_runtime::build_bind_pose_palette(
            skeleton,
            source_to_model,
            &mut bind_palette,
        )?;
        super::validation::validate_player_palette(
            &bind_palette,
            joint_count,
            "validated bind-pose palette",
        )?;
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::PlayerSkinPose {
                palette: bind_palette,
                revision: 1,
            },
        );
        newengine_ulog_api::ulog::info!(
            "game-ready: player bind-pose skin palette validated player={} joints={} policy='YDD skin + YMT hierarchy -> validated model-space identity palette'",
            player.stable_u64(),
            joint_count,
        );
    }

    if let Some(binding) =
        world.get_mut::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
    {
        binding.assignment_revision = assignment.revision;
        binding.source = model_source.clone();
        binding.skeleton_source = skeleton.as_ref().map(|metadata| metadata.source.clone());
        binding.visual_root = Some(visual_root);
        binding.part_count = parts.len() as u32;
        binding.target_height = assignment.target_height;
        binding.feet_to_eye_height = skeleton
            .as_ref()
            .map(|metadata| metadata.anchors.eye_height)
            .unwrap_or(assignment.target_height * assignment.eye_height_ratio);
    }

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
            parts.len()
        ),
    );
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
