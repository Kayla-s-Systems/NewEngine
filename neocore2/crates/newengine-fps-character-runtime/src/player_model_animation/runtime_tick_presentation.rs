include!("runtime_tick_presentation_special.rs");

include!("runtime_tick_presentation_layers.rs");

/// Phase 2: evaluate locomotion and authored presentation layers. This phase owns animation
/// cursors/state machines, but delegates final look/IK/continuity/palette construction to phase 3.
fn evaluate_player_animation_presentation(
    player: newengine_ecs::EntityId,
    binding: &mut PlayerAnimationRuntimeBinding,
    dt: f32,
    frame: &PlayerAnimationFrameInput,
) -> Option<PlayerAnimationFrameOutput> {
    let presentation_started = std::time::Instant::now();
    let semantic = frame.semantic;
    let unarmed_active = frame.unarmed_active;
    let unarmed_attack_sequence = frame.unarmed_attack_sequence;
    let equipment_stance = semantic.equipment_stance;
    let equipment_pose_family = frame.equipment_pose_family.as_deref();
    let equipment_presentation_active = frame.equipment_presentation_active;
    trace_equipment_pose_selection(
        player,
        binding,
        equipment_presentation_active,
        equipment_pose_family,
        equipment_stance,
    );
    let mut timeline_events = Vec::new();
    if semantic.max_pulse_sequence > binding.consumed_pulse_sequence {
        binding.consumed_pulse_sequence = semantic.max_pulse_sequence;
        binding
            .semantic_input
            .discard_pulses_through(binding.consumed_pulse_sequence);
    }
    let mut event_occurrences = Vec::new();
    binding.equipment_time_seconds += dt;
    if equipment_presentation_active {
        begin_or_advance_equipment_transition(binding, equipment_pose_family, equipment_stance, dt);
    } else {
        binding.equipment_previous_stance = EquipmentPresentationStance::None;
        binding.equipment_transition = None;
    }
    binding.equipment_ik_residual_diag_cooldown =
        (binding.equipment_ik_residual_diag_cooldown - dt).max(0.0);
    let relative_rifle_ads_active = equipment_pose_family == Some("rifle")
        && equipment_stance == EquipmentPresentationStance::Aim
        && !frame.first_person_active;
    let visible_sight = binding
        .equipment_resolved_weapon_root
        .zip(frame.weapon_presentation.as_ref())
        .map(|(root, presentation)| crate::weapon_grip::weapon_sight_forward(presentation, root));
    binding.equipment_aim_controller.update(
        relative_rifle_ads_active,
        dt,
        frame.rifle_view_rotation_model,
        visible_sight,
    );
    binding.equipment_resolved_weapon_root = None;
    if unarmed_active {
        if binding.unarmed_attack_sequence != unarmed_attack_sequence {
            binding.unarmed_attack_sequence = unarmed_attack_sequence;
            binding.unarmed_attack_time_seconds = 0.0;
            if let Some(attack) = binding.unarmed_attack_pose.as_mut() {
                attack.event_cursor.restart();
            }
        } else if unarmed_attack_sequence > 0 {
            binding.unarmed_attack_time_seconds += dt;
        }
    } else {
        binding.unarmed_attack_sequence = 0;
        binding.unarmed_attack_time_seconds = 0.0;
    }

    let reload_active = equipment_stance == EquipmentPresentationStance::Reload;
    if reload_active && !binding.equipment_reload_active {
        if let Some(reload) = select_equipment_pose_set_mut(
            &mut binding.equipment_default_pose_set,
            &mut binding.equipment_pose_sets,
            equipment_pose_family,
        )
        .and_then(|set| set.reload.as_mut())
        {
            reload.event_cursor.restart();
        }
    }
    binding.equipment_reload_active = reload_active;

    let (active_state, transitioned, mut clip_ref) =
        evaluate_locomotion_presentation_layer(player, binding, dt, frame, &mut timeline_events)?;
    let turn_step_request = evaluate_native_turn_presentation_layer(
        player,
        binding,
        dt,
        frame,
        active_state,
        &mut clip_ref,
        &mut event_occurrences,
        &mut timeline_events,
    );
    apply_unarmed_presentation_layer(
        player,
        binding,
        frame,
        &mut event_occurrences,
        &mut timeline_events,
    );
    apply_equipment_presentation_layer(
        player,
        binding,
        frame,
        active_state,
        &mut event_occurrences,
        &mut timeline_events,
    );

    apply_special_full_body_overrides(
        player,
        binding,
        dt,
        frame,
        &mut clip_ref,
        &mut timeline_events,
        &mut event_occurrences,
    );

    let presentation_core_ms = presentation_started.elapsed().as_secs_f32() * 1000.0;
    let finalize_started = std::time::Instant::now();
    let (palette, foot_pose, finalize_timing) = finalize_player_pose_and_palette(
        player,
        binding,
        dt,
        frame,
        &clip_ref,
        active_state,
        unarmed_attack_sequence,
        equipment_stance,
        transitioned,
    )?;
    let finalize_ms = finalize_started.elapsed().as_secs_f32() * 1000.0;
    Some(PlayerAnimationFrameOutput {
        palette,
        clip_ref,
        active_state,
        foot_pose,
        turn_step_request,
        model_to_world: frame.model_to_world,
        timeline_events,
        presentation_core_ms,
        finalize_ms,
        finalize_timing,
    })
}
