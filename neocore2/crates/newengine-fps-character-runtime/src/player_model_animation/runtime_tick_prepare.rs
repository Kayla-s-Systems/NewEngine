struct PlayerAnimationFrameInput {
    semantic: PlayerAnimationSemanticFrameState,
    fall_presentation_requested: bool,
    unarmed_active: bool,
    unarmed_attack_sequence: u64,
    first_person_active: bool,
    rifle_secondary_rotation_offset_local: Vec3,
    rifle_view_rotation_model: Option<Quat>,
    rifle_view_forward_model: Option<Vec3>,
    weapon_presentation: Option<newengine_engine_runtime::gameplay::WeaponPresentationDefinition>,
    /// Open-ended equipped-item presentation family (`pistol`, `rifle`, ...).
    equipment_pose_family: Option<String>,
    equipment_presentation_active: bool,
    model_to_world: Mat4,
    first_person_eye_model: Option<Vec3>,
    previous_foot_pose: Option<newengine_model_contact_api::ModelFootPoseState>,
    next_foot_pose_revision: u64,
    root_velocity_local: Vec3,
    aim_velocity_local: Vec3,
    root_position: Vec3,
    root_rotation: Quat,
    body_yaw: f32,
    view_body_yaw_delta: f32,
    view_pitch: f32,
    native_turn_allowed: bool,
}

struct PlayerAnimationFrameOutput {
    palette: Vec<Mat4>,
    clip_ref: String,
    active_state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    foot_pose: Option<newengine_model_contact_api::ModelFootPoseState>,
    turn_step_request: Option<f32>,
    model_to_world: Mat4,
    timeline_events: Vec<newengine_animation_api::AnimationTimelineEventV1>,
    presentation_core_ms: f32,
    finalize_ms: f32,
    finalize_timing: PlayerAnimationFinalizeTiming,
}

/// Phase 1: consume semantic events and snapshot every world-owned input before mutably borrowing
/// the animation binding. No pose state is mutated here.
fn prepare_player_animation_frame(
    world: &mut newengine_ecs::World,
    player: newengine_ecs::EntityId,
) -> Option<PlayerAnimationFrameInput> {
    let semantic_events =
        crate::animation_semantic::semantic_events_for_entity(world, player.stable_u64());
    if let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) {
        if let Err(error) = binding.consume_semantic_events(semantic_events.iter()) {
            newengine_ulog_api::ulog::warn!(
                "fps-character: animation semantic event consume failed player={} err='{}'",
                player.stable_u64(),
                error
            );
        }
    }
    let semantic = world
        .get::<PlayerAnimationRuntimeBinding>(player)
        .map(semantic_frame_state)
        .unwrap_or_else(|| PlayerAnimationSemanticFrameState {
            animation_state: newengine_engine_runtime::gameplay::PlayerAnimationState::default(),
            look_context: newengine_engine_runtime::gameplay::PlayerLookContext::default(),
            view_yaw: None,
            view_pitch: None,
            noclip_enabled: false,
            fall_active: false,
            fall_distance: 0.0,
            equipment_stance: EquipmentPresentationStance::None,
            aim_alpha: 0.0,
            reload_progress: None,
            recoil_alpha: 0.0,
            recoil_yaw_radians: 0.0,
            obstruction_alpha: 0.0,
            unarmed_ready: false,
            unarmed_attack_sequence: 0,
            landing: None,
            max_pulse_sequence: 0,
        });
    let animation_state = semantic.animation_state;
    let noclip_enabled = semantic.noclip_enabled;
    let fall_presentation_requested = authoritative_fall_presentation_requested(
        noclip_enabled,
        semantic.fall_active,
        animation_state.locomotion,
    );
    let active_weapon =
        newengine_engine_runtime::gameplay::active_equipped_weapon_binding(world, player);
    let (prior_unarmed_attack_sequence, prior_unarmed_attack_active) = world
        .get::<PlayerAnimationRuntimeBinding>(player)
        .map(|binding| {
            let active = binding.unarmed_attack_sequence > 0
                && binding.unarmed_attack_pose.as_ref().is_some_and(|clip| {
                    binding.unarmed_attack_time_seconds
                        <= clip.clip.duration_seconds.max(1.0 / 30.0)
                });
            (binding.unarmed_attack_sequence, active)
        })
        .unwrap_or((0, false));
    let unarmed_active = !noclip_enabled
        && !fall_presentation_requested
        && (semantic.unarmed_ready
            || semantic.unarmed_attack_sequence > 0
            || prior_unarmed_attack_active);
    let unarmed_attack_sequence = if !unarmed_active {
        0
    } else if semantic.unarmed_attack_sequence > 0 {
        semantic.unarmed_attack_sequence
    } else if prior_unarmed_attack_active {
        prior_unarmed_attack_sequence
    } else {
        0
    };
    let rifle_aim_alpha = semantic.aim_alpha;
    let first_person_active = world
        .get::<newengine_engine_runtime::gameplay::PlayerActor>(player)
        .is_some()
        && world
            .resource::<newengine_engine_runtime::gameplay::PlayerViewState>()
            .copied()
            .unwrap_or_default()
            .first_person_active;
    // Secondary weapon inertia remains physical presentation state. It never selects an
    // animation; the semantic equipment event above owns Ready/Aim/Reload selection.
    let rifle_secondary_rotation_offset_local = if first_person_active {
        Vec3::ZERO
    } else {
        super::equipment_visual::equipped_weapon_secondary_rotation_offset_local(world, player)
    };
    let rifle_view_rotation_model = if first_person_active || rifle_aim_alpha > 0.001 {
        player_rifle_view_rotation_model(world, player)
    } else {
        None
    };
    let rifle_view_forward_model = rifle_view_rotation_model
        .map(|rotation| (rotation * -Vec3::Z).normalize_or_zero())
        .filter(|forward| forward.is_finite() && forward.length_squared() > 1.0e-8);
    let (weapon_presentation, equipment_pose_family) = active_weapon
        .and_then(|equipped| {
            world
                .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()?
                .get(equipped.item)
                .map(|definition| {
                    (
                        Some(definition.weapon_presentation.clone().sanitized())
                            .filter(|presentation| presentation.enabled),
                        definition.weapon_class.clone(),
                    )
                })
        })
        .unwrap_or((None, None));
    let equipment_stance = semantic.equipment_stance;
    let equipment_presentation_active = !noclip_enabled
        && !fall_presentation_requested
        && equipment_stance != EquipmentPresentationStance::None
        && world
            .get::<PlayerAnimationRuntimeBinding>(player)
            .is_some_and(|binding| {
                binding.has_equipment_pose_for_family(equipment_pose_family.as_deref())
                    || (weapon_presentation.is_some() && binding.equipment_ik.is_some())
            });
    let world_velocity = world
        .get::<newengine_sim::Velocity>(player)
        .copied()
        .unwrap_or_default()
        .0;

    let root_transform = world.get::<Transform>(player).copied().unwrap_or_default();
    let model_root_local = world
        .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
        .and_then(|binding| binding.visual_root)
        .and_then(|visual_root| world.get::<Transform>(visual_root).copied())
        .unwrap_or_default();
    // Gameplay facing is the PlayerActor root. `visual_root.rotation` is an imported-model basis
    // correction (Ellie is a good example with yaw_offset=PI) and must never be interpreted as
    // additional gameplay yaw, otherwise an aligned character appears to require a 180° turn.
    let rendered_body_rotation = root_transform.rotation.normalize_or_identity();
    let body_forward = (rendered_body_rotation * -Vec3::Z).normalize_or_zero();
    let body_yaw = if body_forward.length_squared() > 1.0e-8 {
        (-body_forward.x).atan2(-body_forward.z)
    } else {
        0.0
    };
    let view_yaw = semantic
        .view_yaw
        .filter(|yaw| yaw.is_finite())
        .unwrap_or(body_yaw);
    let view_pitch = semantic
        .view_pitch
        .filter(|pitch| pitch.is_finite())
        .unwrap_or(0.0);
    let view_body_yaw_delta = newengine_math::wrap_pi(view_yaw - body_yaw);
    let horizontal_speed = Vec3::new(world_velocity.x, 0.0, world_velocity.z).length();
    let native_turn_allowed = !noclip_enabled
        && !fall_presentation_requested
        && horizontal_speed < 0.08
        && animation_state.locomotion
            == newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Idle;
    let model_to_world = root_transform.to_mat4() * model_root_local.to_mat4();
    let first_person_eye_model = if first_person_active {
        world
            .get::<newengine_engine_runtime::gameplay::PlayerFirstPersonCameraAnchor>(player)
            .copied()
            .filter(|anchor| anchor.eye_center_ws.is_finite())
            .map(|anchor| {
                model_to_world
                    .inverse()
                    .transform_point3(anchor.eye_center_ws)
            })
            .filter(|position| position.is_finite())
    } else {
        None
    };
    let previous_foot_pose = world
        .get::<newengine_model_contact_api::ModelFootPoseState>(player)
        .copied();
    let next_foot_pose_revision = previous_foot_pose
        .map(|pose| pose.revision.saturating_add(1).max(1))
        .unwrap_or(1);
    let root_velocity_local = root_transform.rotation.inverse() * world_velocity;
    // Weapon aim locomotion is authored relative to the view/aim heading, not the body/world axes.
    // Rotating body-local velocity by the inverse live view-body delta gives exactly that space.
    let mut aim_velocity_local = Quat::from_rotation_y(-view_body_yaw_delta) * root_velocity_local;
    aim_velocity_local.y = 0.0;
    let root_position = root_transform.position;
    let root_rotation = root_transform.rotation;

    Some(PlayerAnimationFrameInput {
        semantic,
        fall_presentation_requested,
        unarmed_active,
        unarmed_attack_sequence,
        first_person_active,
        rifle_secondary_rotation_offset_local,
        rifle_view_rotation_model,
        rifle_view_forward_model,
        weapon_presentation,
        equipment_pose_family,
        equipment_presentation_active,
        model_to_world,
        first_person_eye_model,
        previous_foot_pose,
        next_foot_pose_revision,
        root_velocity_local,
        aim_velocity_local,
        root_position,
        root_rotation,
        body_yaw,
        view_body_yaw_delta,
        view_pitch,
        native_turn_allowed,
    })
}
