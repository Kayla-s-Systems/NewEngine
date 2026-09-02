fn player_motion_response_from_ytyp(
    player: &serde_json::Value,
) -> Option<PlayerMotionResponseData> {
    let response = value_path(player, &["motion_response"])?;
    if !response.is_object() {
        return None;
    }
    Some(PlayerMotionResponseData {
        velocity_spring_const: value_path(response, &["velocity_spring_const"])
            .and_then(value_f32)?,
        velocity_spring_const_decel: value_path(response, &["velocity_spring_const_decel"])
            .and_then(value_f32)?,
        velocity_spring_dampen_ratio: value_path(response, &["velocity_spring_dampen_ratio"])
            .and_then(value_f32)?,
        speed_spring_const: value_path(response, &["speed_spring_const"]).and_then(value_f32)?,
        max_accel: value_path(response, &["max_accel"]).and_then(value_f32)?,
        trans_clamp_dist: value_path(response, &["trans_clamp_dist"]).and_then(value_f32)?,
    })
}

fn apply_player_runtime_data_from_ytyp(
    profile: &mut AuthoredWorldProfile,
    data: &mut GameData,
    metadata: &serde_json::Value,
) -> usize {
    let Some(player) = value_path(metadata, &["player"]) else {
        return 0;
    };
    if !player.is_object() {
        return 0;
    }

    // The Shared character definition is authoritative for character-owned model and locomotion
    // data. Project GameData remains authoritative for spawn/look and world physics.
    data.player.model.enabled = profile.player.model.enabled;
    data.player.model.source = profile.player.model.source.clone();
    data.player.model.target_height = profile.player.model.target_height;
    data.player.model.eye_height_ratio = profile.player.model.eye_height_ratio;
    data.player.model.local_offset = [
        profile.player.model.local_offset.x,
        profile.player.model.local_offset.y,
        profile.player.model.local_offset.z,
    ];
    data.player.model.yaw_offset = profile.player.model.yaw_offset;
    data.player.model.hide_in_first_person = profile.player.model.hide_in_first_person;
    data.player.move_speed = profile.player.run_speed;

    let tuning = &mut data.player.tuning;
    let mut applied = 1usize;
    if value_path(player, &["motion_response"]).is_some() {
        if let Some(response) = player_motion_response_from_ytyp(player) {
            tuning.motion_response = Some(response);
            applied += 6;
            newengine_ulog_api::ulog::info!(
                "game-ready ytyp player motion_response: velocity_k={:.3} decel_k={:.3} dampen={:.3} speed_k={:.3} max_accel={:.3} trans_clamp_dist={:.4} policy='authored spring/K payload; max_accel sentinel semantics unresolved'",
                response.velocity_spring_const,
                response.velocity_spring_const_decel,
                response.velocity_spring_dampen_ratio,
                response.speed_spring_const,
                response.max_accel,
                response.trans_clamp_dist,
            );
        } else {
            newengine_ulog_api::ulog::warn!(
                "game-ready ytyp player motion_response ignored: block must provide all six finite authored fields"
            );
        }
    }
    macro_rules! apply_tuning {
        ($key:literal, $field:ident, $min:expr, $max:expr) => {
            if let Some(value) = value_path(player, &[$key]).and_then(value_f32) {
                tuning.$field = value.clamp($min, $max);
                applied += 1;
            }
        };
    }

    apply_tuning!("body_radius", body_radius, 0.15, 1.0);
    apply_tuning!("body_half_height", body_half_height, 0.15, 1.5);
    apply_tuning!(
        "crouched_body_half_height",
        crouched_body_half_height,
        0.05,
        1.5
    );
    apply_tuning!("visual_radius", visual_radius, 0.15, 1.0);
    apply_tuning!("visual_half_height", visual_half_height, 0.15, 1.5);
    apply_tuning!("camera_eye_height", camera_eye_height, 0.05, 2.5);
    apply_tuning!(
        "crouched_camera_eye_height",
        crouched_camera_eye_height,
        0.05,
        2.5
    );
    apply_tuning!("crouch_camera_speed", crouch_camera_speed, 0.1, 100.0);
    apply_tuning!("jump_speed", jump_speed, 0.0, 30.0);
    apply_tuning!("ground_probe_distance", ground_probe_distance, 0.01, 2.0);
    apply_tuning!("max_slope_degrees", max_slope_degrees, 0.0, 89.0);
    apply_tuning!("footstep_stride", footstep_stride, 0.25, 10.0);
    apply_tuning!(
        "landing_speed_threshold",
        landing_speed_threshold,
        0.0,
        100.0
    );
    apply_tuning!(
        "locomotion_min_horizontal_speed",
        locomotion_min_horizontal_speed,
        0.0,
        20.0
    );
    apply_tuning!(
        "ground_probe_max_upward_velocity",
        ground_probe_max_upward_velocity,
        -20.0,
        20.0
    );
    apply_tuning!(
        "landing_min_airborne_seconds",
        landing_min_airborne_seconds,
        0.0,
        5.0
    );

    if let Some(value) = value_path(player, &["sprint_multiplier"]).and_then(value_f32) {
        tuning.sprint_multiplier = value.clamp(1.0, 8.0);
        applied += 1;
    } else if profile.player.run_speed > 0.0 {
        tuning.sprint_multiplier =
            (profile.player.sprint_speed / profile.player.run_speed).clamp(1.0, 8.0);
    }

    profile.gameplay.player_collision.radius = tuning.body_radius;
    profile.gameplay.player_collision.half_height = tuning.body_half_height;
    profile.gameplay.player_visual.radius = tuning.visual_radius;
    profile.gameplay.player_visual.half_height = tuning.visual_half_height;
    profile.gameplay.player_visual.camera_eye_height = tuning.camera_eye_height;
    profile.gameplay.player_visual.sprint_multiplier = tuning.sprint_multiplier;
    applied
}
