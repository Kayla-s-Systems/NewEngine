fn apply_player_animation_metadata(
    profile: &mut AuthoredWorldProfile,
    model: &serde_json::Value,
) -> usize {
    let mut applied = 0usize;
    if let Some(reference) = value_path(model, &["idle_animation"]).and_then(value_string) {
        profile.player.model.idle_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["walk_animation"]).and_then(value_string) {
        profile.player.model.walk_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["run_animation"]).and_then(value_string) {
        profile.player.model.run_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["sprint_animation"]).and_then(value_string) {
        profile.player.model.sprint_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["crouch_idle_animation"]).and_then(value_string) {
        profile.player.model.crouch_idle_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["crouch_walk_animation"]).and_then(value_string) {
        profile.player.model.crouch_walk_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["jump_animation"]).and_then(value_string) {
        profile.player.model.jump_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["fall_animation"]).and_then(value_string) {
        profile.player.model.fall_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["fall_low_animation"]).and_then(value_string) {
        profile.player.model.fall_low_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["fall_medium_animation"]).and_then(value_string) {
        profile.player.model.fall_medium_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["fall_high_animation"]).and_then(value_string) {
        profile.player.model.fall_high_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["landing_soft_animation"]).and_then(value_string) {
        profile.player.model.landing_soft_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["landing_medium_animation"]).and_then(value_string)
    {
        profile.player.model.landing_medium_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["landing_hard_animation"]).and_then(value_string) {
        profile.player.model.landing_hard_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) =
        value_path(model, &["landing_hard_run_animation"]).and_then(value_string)
    {
        profile.player.model.landing_hard_run_animation = Some(reference);
        applied += 1;
    }
    if let Some(value) = value_path(model, &["fall_medium_min_distance"]).and_then(value_f32) {
        profile.player.model.fall_medium_min_distance = value.max(0.0);
        applied += 1;
    }
    if let Some(value) = value_path(model, &["fall_high_min_distance"]).and_then(value_f32) {
        profile.player.model.fall_high_min_distance =
            value.max(profile.player.model.fall_medium_min_distance);
        applied += 1;
    }
    if let Some(reference) =
        value_path(model, &["equipment_ready_animation"]).and_then(value_string)
    {
        profile.player.model.equipment_ready_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["equipment_aim_animation"]).and_then(value_string)
    {
        profile.player.model.equipment_aim_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) =
        value_path(model, &["equipment_reload_animation"]).and_then(value_string)
    {
        profile.player.model.equipment_reload_animation = Some(reference);
        applied += 1;
    }
    // Character-owned equipment pose families are open-ended. The selected item's authored
    // `weapon.class` chooses one of these slots at runtime; this parser must therefore discover
    // families from metadata instead of encoding a knife/rifle/pistol enum here.
    if let Some(attributes) = model.as_object() {
        for (attribute, value) in attributes {
            if let Some(family) = equipment_ready_sample_phase_family_from_attribute(attribute) {
                if let Some(phase) = value_f32(value) {
                    profile
                        .player
                        .model
                        .equipment_ready_sample_phases
                        .insert(family, phase.clamp(0.0, 1.0));
                    applied += 1;
                }
                continue;
            }
            let Some(semantic) = equipment_animation_slot_from_attribute(attribute) else {
                continue;
            };
            let Some(reference) = value_string(value) else {
                continue;
            };
            profile
                .player
                .model
                .animation_slots
                .insert(semantic, reference);
            applied += 1;
        }
    }

    for (attribute, semantic) in [
        ("look_relaxed_base_animation", "look.relaxed.base"),
        ("look_relaxed_range_animation", "look.relaxed.range"),
        ("look_crouch_base_animation", "look.crouch.base"),
        ("look_crouch_range_animation", "look.crouch.range"),
        ("look_tense_base_animation", "look.tense.base"),
        ("look_tense_range_animation", "look.tense.range"),
        ("look_eyes_base_animation", "look.eyes.base"),
        ("look_eyes_range_animation", "look.eyes.range"),
        (
            "look_context_cover_low_left_base_animation",
            "look.context.cover_low_left.base",
        ),
        (
            "look_context_cover_low_left_range_animation",
            "look.context.cover_low_left.range",
        ),
        (
            "look_context_cover_low_right_base_animation",
            "look.context.cover_low_right.base",
        ),
        (
            "look_context_cover_low_right_range_animation",
            "look.context.cover_low_right.range",
        ),
        (
            "look_context_prone_base_animation",
            "look.context.prone.base",
        ),
        (
            "look_context_prone_range_animation",
            "look.context.prone.range",
        ),
        (
            "look_context_supine_base_animation",
            "look.context.supine.base",
        ),
        (
            "look_context_supine_range_animation",
            "look.context.supine.range",
        ),
        ("look_context_rope_base_animation", "look.context.rope.base"),
        (
            "look_context_rope_range_animation",
            "look.context.rope.range",
        ),
        (
            "look_context_ladder_base_animation",
            "look.context.ladder.base",
        ),
        (
            "look_context_ladder_range_animation",
            "look.context.ladder.range",
        ),
        (
            "look_context_swim_idle_base_animation",
            "look.context.swim_idle.base",
        ),
        (
            "look_context_swim_idle_range_animation",
            "look.context.swim_idle.range",
        ),
        (
            "look_context_injured_base_animation",
            "look.context.injured.base",
        ),
        (
            "look_context_injured_range_animation",
            "look.context.injured.range",
        ),
        (
            "look_context_relaxed_injured_base_animation",
            "look.context.relaxed_injured.base",
        ),
        (
            "look_context_relaxed_injured_range_animation",
            "look.context.relaxed_injured.range",
        ),
    ] {
        if let Some(reference) = value_path(model, &[attribute]).and_then(value_string) {
            profile
                .player
                .model
                .animation_slots
                .insert(semantic.to_owned(), reference);
            applied += 1;
        }
    }
    if let Some(phase) = value_path(model, &["equipment_ready_sample_phase"]).and_then(value_f32) {
        profile.player.model.equipment_ready_sample_phase = phase.clamp(0.0, 1.0);
        applied += 1;
    }
    if let Some(weights) = value_path(model, &["equipment_ready_rotation_weights"])
        .and_then(player_joint_rotation_weights)
    {
        profile.player.model.equipment_ready_rotation_weights = weights;
        applied += 1;
    }
    if let Some(weights) = value_path(model, &["equipment_aim_rotation_weights"])
        .and_then(player_joint_rotation_weights)
    {
        profile.player.model.equipment_aim_rotation_weights = weights;
        applied += 1;
    }
    if let Some(weights) = value_path(model, &["equipment_reload_rotation_weights"])
        .and_then(player_joint_rotation_weights)
    {
        profile.player.model.equipment_reload_rotation_weights = weights;
        applied += 1;
    }
    if let Some(enabled) = value_path(model, &["equipment_arm_ik"]).and_then(value_bool) {
        profile.player.model.equipment_arm_ik = enabled;
        applied += 1;
    }
    let ik_required = [
        "equipment_arm_ik_chest",
        "equipment_arm_ik_right_shoulder",
        "equipment_arm_ik_right_elbow",
        "equipment_arm_ik_right_wrist",
        "equipment_arm_ik_right_palm",
        "equipment_arm_ik_left_shoulder",
        "equipment_arm_ik_left_elbow",
        "equipment_arm_ik_left_wrist",
        "equipment_arm_ik_left_palm",
    ]
    .map(|key| value_path(model, &[key]).and_then(value_string));
    if let [Some(chest), Some(right_shoulder), Some(right_elbow), Some(right_wrist), Some(right_palm), Some(left_shoulder), Some(left_elbow), Some(left_wrist), Some(left_palm)] =
        ik_required
    {
        profile.player.model.equipment_arm_ik_rig = Some(
            newengine_engine_runtime::gameplay::PlayerWeaponArmIkRigDefinition {
                chest,
                right_shoulder,
                right_elbow,
                right_wrist,
                right_palm,
                right_prop_attachment: value_path(
                    model,
                    &["equipment_arm_ik_right_prop_attachment"],
                )
                .and_then(value_string),
                left_shoulder,
                left_elbow,
                left_wrist,
                left_palm,
                left_prop_attachment: value_path(model, &["equipment_arm_ik_left_prop_attachment"])
                    .and_then(value_string),
            },
        );
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["unarmed_ready_animation"]).and_then(value_string)
    {
        profile.player.model.unarmed_ready_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["unarmed_attack_animation"]).and_then(value_string)
    {
        profile.player.model.unarmed_attack_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_45_left_animation"]).and_then(value_string) {
        profile.player.model.turn_45_left_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_45_right_animation"]).and_then(value_string)
    {
        profile.player.model.turn_45_right_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_90_left_animation"]).and_then(value_string) {
        profile.player.model.turn_90_left_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_90_right_animation"]).and_then(value_string)
    {
        profile.player.model.turn_90_right_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_135_left_animation"]).and_then(value_string)
    {
        profile.player.model.turn_135_left_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_135_right_animation"]).and_then(value_string)
    {
        profile.player.model.turn_135_right_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_180_left_animation"]).and_then(value_string)
    {
        profile.player.model.turn_180_left_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_180_right_animation"]).and_then(value_string)
    {
        profile.player.model.turn_180_right_animation = Some(reference);
        applied += 1;
    }
    // Normalize legacy per-animation attributes into the project-owned slot table. Runtime
    // consumers resolve only these semantic slots; filenames/clip names never escape authoring.
    for (slot, reference) in [
        (
            "locomotion.idle",
            profile.player.model.idle_animation.clone(),
        ),
        (
            "locomotion.walk",
            profile.player.model.walk_animation.clone(),
        ),
        ("locomotion.run", profile.player.model.run_animation.clone()),
        (
            "locomotion.sprint",
            profile.player.model.sprint_animation.clone(),
        ),
        (
            "locomotion.crouch_idle",
            profile.player.model.crouch_idle_animation.clone(),
        ),
        (
            "locomotion.crouch_walk",
            profile.player.model.crouch_walk_animation.clone(),
        ),
        (
            "locomotion.jump",
            profile.player.model.jump_animation.clone(),
        ),
        (
            "locomotion.fall",
            profile.player.model.fall_animation.clone(),
        ),
        ("fall.low", profile.player.model.fall_low_animation.clone()),
        (
            "fall.medium",
            profile.player.model.fall_medium_animation.clone(),
        ),
        (
            "fall.high",
            profile.player.model.fall_high_animation.clone(),
        ),
        (
            "landing.soft",
            profile.player.model.landing_soft_animation.clone(),
        ),
        (
            "landing.medium",
            profile.player.model.landing_medium_animation.clone(),
        ),
        (
            "landing.hard",
            profile.player.model.landing_hard_animation.clone(),
        ),
        (
            "landing.hard_run",
            profile.player.model.landing_hard_run_animation.clone(),
        ),
        (
            "equipment.ready",
            profile.player.model.equipment_ready_animation.clone(),
        ),
        (
            "equipment.aim",
            profile.player.model.equipment_aim_animation.clone(),
        ),
        (
            "equipment.reload",
            profile.player.model.equipment_reload_animation.clone(),
        ),
        (
            "unarmed.ready",
            profile.player.model.unarmed_ready_animation.clone(),
        ),
        (
            "unarmed.attack",
            profile.player.model.unarmed_attack_animation.clone(),
        ),
        (
            "turn.left.45",
            profile.player.model.turn_45_left_animation.clone(),
        ),
        (
            "turn.right.45",
            profile.player.model.turn_45_right_animation.clone(),
        ),
        (
            "turn.left.90",
            profile.player.model.turn_90_left_animation.clone(),
        ),
        (
            "turn.right.90",
            profile.player.model.turn_90_right_animation.clone(),
        ),
        (
            "turn.left.135",
            profile.player.model.turn_135_left_animation.clone(),
        ),
        (
            "turn.right.135",
            profile.player.model.turn_135_right_animation.clone(),
        ),
        (
            "turn.left.180",
            profile.player.model.turn_180_left_animation.clone(),
        ),
        (
            "turn.right.180",
            profile.player.model.turn_180_right_animation.clone(),
        ),
    ] {
        if let Some(reference) = reference {
            profile
                .player
                .model
                .animation_slots
                .insert(slot.to_owned(), reference);
        }
    }
    if let Some(reference) = value_path(model, &["noclip_animation"]).and_then(value_string) {
        profile
            .player
            .model
            .animation_slots
            .insert("traversal.noclip".to_owned(), reference);
    }

    applied
}
