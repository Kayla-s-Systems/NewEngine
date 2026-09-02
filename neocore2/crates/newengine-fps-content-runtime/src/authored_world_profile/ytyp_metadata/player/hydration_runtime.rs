fn apply_player_runtime_tuning(
    profile: &mut AuthoredWorldProfile,
    player_values: &serde_json::Value,
) -> usize {
    let mut applied = 0usize;
    if let Some(value) = value_path(player_values, &["walk_speed"]).and_then(value_f32) {
        profile.player.walk_speed = value.clamp(0.05, 50.0);
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["run_speed"]).and_then(value_f32) {
        profile.player.run_speed = value.clamp(0.05, 50.0);
        profile.player.move_speed = profile.player.run_speed;
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["sprint_speed"]).and_then(value_f32) {
        profile.player.sprint_speed = value.clamp(0.05, 75.0);
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["crouch_speed"]).and_then(value_f32) {
        profile.player.crouch_speed = value.clamp(0.05, 50.0);
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["combat_team"]).and_then(value_f32) {
        let team = value.round();
        if (1.0..=65_535.0).contains(&team) {
            profile.player.combat_team = Some(team as u32);
            applied += 1;
        }
    }
    if let Some(value) = value_path(player_values, &["health_maximum"]).and_then(value_f32) {
        profile.player.health_maximum = value.clamp(1.0, 1_000_000.0);
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["stamina_maximum"]).and_then(value_f32) {
        profile.player.stamina_maximum = value.clamp(0.0, 1_000_000.0);
        applied += 1;
    }
    if let Some(value) =
        value_path(player_values, &["stamina_sprint_drain_per_second"]).and_then(value_f32)
    {
        profile.player.stamina_sprint_drain_per_second = value.clamp(0.0, 10_000.0);
        applied += 1;
    }
    if let Some(value) =
        value_path(player_values, &["stamina_regen_per_second"]).and_then(value_f32)
    {
        profile.player.stamina_regen_per_second = value.clamp(0.0, 10_000.0);
        applied += 1;
    }
    if let Some(value) =
        value_path(player_values, &["stamina_regen_delay_seconds"]).and_then(value_f32)
    {
        profile.player.stamina_regen_delay_seconds = value.clamp(0.0, 60.0);
        applied += 1;
    }
    if let Some(value) =
        value_path(player_values, &["stamina_exhausted_resume_fraction"]).and_then(value_f32)
    {
        profile.player.stamina_exhausted_resume_fraction = value.clamp(0.0, 1.0);
        applied += 1;
    }
    if let Some(value) =
        value_path(player_values, &["damage_stagger_damage_fraction"]).and_then(value_f32)
    {
        profile
            .player
            .damage_response_tuning
            .stagger_damage_fraction = value.clamp(0.0, 1.0);
        applied += 1;
    }
    if let Some(value) =
        value_path(player_values, &["damage_stagger_impulse_threshold"]).and_then(value_f32)
    {
        profile
            .player
            .damage_response_tuning
            .stagger_impulse_threshold = value.clamp(0.0, 100_000.0);
        applied += 1;
    }
    if let Some(value) =
        value_path(player_values, &["damage_flinch_duration_seconds"]).and_then(value_f32)
    {
        profile
            .player
            .damage_response_tuning
            .flinch_duration_seconds = value.clamp(0.0, 10.0);
        applied += 1;
    }
    if let Some(value) =
        value_path(player_values, &["damage_stagger_duration_seconds"]).and_then(value_f32)
    {
        profile
            .player
            .damage_response_tuning
            .stagger_duration_seconds = value.clamp(0.0, 10.0);
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["injured_health_fraction"]).and_then(value_f32)
    {
        profile
            .player
            .damage_response_tuning
            .injured_health_fraction = value.clamp(0.0, 1.0);
        applied += 1;
    }
    profile.player.damage_response_tuning = profile.player.damage_response_tuning.sanitized();
    if let Some(value) =
        value_path(player_values, &["drop_active_weapon_on_death"]).and_then(value_bool)
    {
        profile.player.death_policy.drop_active_weapon = value;
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["death_presentation"]).and_then(value_string) {
        profile.player.death_policy.presentation = match value.trim().to_ascii_lowercase().as_str()
        {
            "animation" => {
                newengine_engine_runtime::gameplay::CharacterDeathPresentation::Animation
            }
            "ragdoll" => newengine_engine_runtime::gameplay::CharacterDeathPresentation::Ragdoll,
            _ => {
                newengine_engine_runtime::gameplay::CharacterDeathPresentation::AnimationThenRagdoll
            }
        };
        applied += 1;
    }
    // Sanitize the set as one unit so an authored typo cannot invert movement modes.
    profile.player.walk_speed = profile.player.walk_speed.min(profile.player.run_speed);
    profile.player.sprint_speed = profile.player.sprint_speed.max(profile.player.run_speed);
    profile.player.crouch_speed = profile.player.crouch_speed.min(profile.player.run_speed);

    applied
}
