use super::*;

pub(super) fn to_fps_demo_rules(
    spec: &GameReadyGameplaySpec,
    model: &self::content::GameReadyPlayerModelSpec,
    game_data: &GameData,
) -> FpsDemoRules {
    let tuning = game_data.player.tuning;
    let default_player = FpsPlayerTuning {
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
    }
    .sanitized();
    let base = FpsPlayerTuning {
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
    }
    .sanitized();
    let feet_to_eye = model.target_height * model.eye_height_ratio;
    let model_eye_offset_from_player_origin =
        feet_to_eye - (base.body_half_height + base.body_radius);
    let player = FpsPlayerTuning {
        camera_eye_height: model_eye_offset_from_player_origin.clamp(0.05, model.target_height),
        ..base
    }
    .sanitized();

    FpsDemoRules {
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
