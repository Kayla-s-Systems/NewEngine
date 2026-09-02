use super::*;

#[inline]
fn fps_motion_response_from_game_data(
    response: Option<newengine_game_data::PlayerMotionResponseData>,
) -> Option<FpsMotionResponseTuning> {
    response.map(|response| FpsMotionResponseTuning {
        velocity_spring_const: response.velocity_spring_const,
        velocity_spring_const_decel: response.velocity_spring_const_decel,
        velocity_spring_dampen_ratio: response.velocity_spring_dampen_ratio,
        speed_spring_const: response.speed_spring_const,
        max_accel: response.max_accel,
        trans_clamp_dist: response.trans_clamp_dist,
    })
}

pub(super) fn to_fps_demo_rules(
    spec: &GameReadyGameplaySpec,
    game_data: &GameData,
) -> FpsRuntimeRules {
    let tuning = game_data.player.tuning;
    let default_player = FpsPlayerTuning {
        motion_response: fps_motion_response_from_game_data(tuning.motion_response),
        body_radius: tuning.body_radius,
        body_half_height: tuning.body_half_height,
        crouched_body_half_height: tuning.crouched_body_half_height,
        visual_radius: tuning.visual_radius,
        visual_half_height: tuning.visual_half_height,
        camera_eye_height: tuning.camera_eye_height,
        crouched_camera_eye_height: tuning.crouched_camera_eye_height,
        crouch_camera_speed: tuning.crouch_camera_speed,
        sprint_multiplier: tuning.sprint_multiplier,
        jump_speed: tuning.jump_speed,
        gravity: tuning.gravity,
        contact_skin: tuning.contact_skin,
        ground_probe_distance: tuning.ground_probe_distance,
        max_slope_radians: tuning.max_slope_degrees.to_radians(),
        footstep_stride: tuning.footstep_stride,
        landing_speed_threshold: tuning.landing_speed_threshold,
        locomotion_min_horizontal_speed: tuning.locomotion_min_horizontal_speed,
        ground_probe_max_upward_velocity: tuning.ground_probe_max_upward_velocity,
        landing_min_airborne_seconds: tuning.landing_min_airborne_seconds,
    }
    .sanitized();
    let base = FpsPlayerTuning {
        motion_response: default_player.motion_response,
        body_radius: spec.player_collision.radius,
        body_half_height: spec.player_collision.half_height,
        crouched_body_half_height: default_player.crouched_body_half_height,
        visual_radius: spec.player_visual.radius,
        visual_half_height: spec.player_visual.half_height,
        camera_eye_height: spec.player_visual.camera_eye_height,
        crouched_camera_eye_height: default_player.crouched_camera_eye_height,
        crouch_camera_speed: default_player.crouch_camera_speed,
        sprint_multiplier: spec.player_visual.sprint_multiplier,
        jump_speed: default_player.jump_speed,
        gravity: spec.physics.gravity,
        contact_skin: spec.physics.contact_skin,
        ground_probe_distance: default_player.ground_probe_distance,
        max_slope_radians: default_player.max_slope_radians,
        footstep_stride: default_player.footstep_stride,
        landing_speed_threshold: default_player.landing_speed_threshold,
        locomotion_min_horizontal_speed: default_player.locomotion_min_horizontal_speed,
        ground_probe_max_upward_velocity: default_player.ground_probe_max_upward_velocity,
        landing_min_airborne_seconds: default_player.landing_min_airborne_seconds,
    }
    .sanitized();
    let player = base;

    FpsRuntimeRules {
        default_status: spec.default_status.clone(),
        pickup_status: spec.pickup_status.clone(),
        target_status: spec.target_status.clone(),
        hazard_status: spec.hazard_status.clone(),
        goal_locked_status: spec.goal_locked_status.clone(),
        goal_complete_status: spec.goal_complete_status.clone(),
        failed_progress_label: spec.failed_progress_label.clone(),
        completed_progress_label: spec.completed_progress_label.clone(),
        player,
    }
}

#[cfg(test)]
mod motion_response_bridge_tests {
    use super::*;

    #[test]
    fn game_data_motion_response_maps_to_fps_runtime_verbatim() {
        let source = newengine_game_data::PlayerMotionResponseData {
            velocity_spring_const: 7.0,
            velocity_spring_const_decel: 10.0,
            velocity_spring_dampen_ratio: 1.0,
            speed_spring_const: 4.6,
            max_accel: -1.0,
            trans_clamp_dist: 0.01,
        };
        let mapped = fps_motion_response_from_game_data(Some(source)).expect("runtime response");
        assert_eq!(mapped.velocity_spring_const, 7.0);
        assert_eq!(mapped.velocity_spring_const_decel, 10.0);
        assert_eq!(mapped.velocity_spring_dampen_ratio, 1.0);
        assert_eq!(mapped.speed_spring_const, 4.6);
        assert_eq!(mapped.max_accel, -1.0);
        assert_eq!(mapped.trans_clamp_dist, 0.01);
        assert_eq!(fps_motion_response_from_game_data(None), None);
    }
}
