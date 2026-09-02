fn apply_player_model_view_metadata(
    profile: &mut AuthoredWorldProfile,
    model: &serde_json::Value,
) -> usize {
    let mut applied = 0usize;
    if let Some(visibility) = value_path(model, &["visibility"]).and_then(value_string) {
        let visibility = visibility.to_ascii_lowercase();
        profile.player.model.hide_in_first_person = visibility.contains("hide_in_first_person")
            || visibility.contains("first_person_hidden");
        applied += 1;
    }
    if let Some(target_height) = value_path(model, &["target_height"]).and_then(value_f32) {
        profile.player.model.target_height = target_height.clamp(0.25, 3.0);
        applied += 1;
    }
    if let Some(eye_height_ratio) = value_path(model, &["eye_height_ratio"]).and_then(value_f32) {
        profile.player.model.eye_height_ratio = eye_height_ratio.clamp(0.55, 0.98);
        applied += 1;
    }
    if let Some(yaw_offset) = value_path(model, &["yaw_offset"]).and_then(value_f32) {
        profile.player.model.yaw_offset = yaw_offset;
        applied += 1;
    }
    applied
}
