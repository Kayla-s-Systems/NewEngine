pub fn apply_gameplay_constants_from_ytyp(
    profile: &mut AuthoredWorldProfile,
    metadata: &serde_json::Value,
) -> usize {
    let mut applied = 0usize;
    if let Some(radius) =
        value_path(metadata, &["gameplay", "player_collision", "radius"]).and_then(value_f32)
    {
        profile.gameplay.player_collision.radius = radius.clamp(0.15, 1.0);
        applied += 1;
    }
    if let Some(half_height) =
        value_path(metadata, &["gameplay", "player_collision", "half_height"]).and_then(value_f32)
    {
        profile.gameplay.player_collision.half_height = half_height.clamp(0.15, 1.5);
        applied += 1;
    }
    if let Some(radius) =
        value_path(metadata, &["gameplay", "player_visual", "radius"]).and_then(value_f32)
    {
        profile.gameplay.player_visual.radius = radius.clamp(0.15, 1.0);
        applied += 1;
    }
    if let Some(half_height) =
        value_path(metadata, &["gameplay", "player_visual", "half_height"]).and_then(value_f32)
    {
        profile.gameplay.player_visual.half_height = half_height.clamp(0.15, 1.5);
        applied += 1;
    }
    if let Some(camera_eye_height) = value_path(
        metadata,
        &["gameplay", "player_visual", "camera_eye_height"],
    )
    .and_then(value_f32)
    {
        profile.gameplay.player_visual.camera_eye_height = camera_eye_height.clamp(0.05, 2.5);
        applied += 1;
    }
    if let Some(sprint_multiplier) = value_path(
        metadata,
        &["gameplay", "player_visual", "sprint_multiplier"],
    )
    .and_then(value_f32)
    {
        profile.gameplay.player_visual.sprint_multiplier = sprint_multiplier.clamp(1.0, 4.0);
        applied += 1;
    }
    applied
}

pub fn apply_sky_constants_from_ytyp(
    profile: &mut AuthoredWorldProfile,
    metadata: &serde_json::Value,
) -> usize {
    let mut applied = 0usize;
    if let Some(radius) = value_path(metadata, &["sky", "radius"]).and_then(value_f32) {
        profile.sky.radius = radius.max(16.0);
        applied += 1;
    }
    if let Some(sun_radius) = value_path(metadata, &["sky", "sun_radius"]).and_then(value_f32) {
        profile.sky.sun_radius = sun_radius.clamp(1.0, 64.0);
        applied += 1;
    }
    if let Some(moon_radius) = value_path(metadata, &["sky", "moon_radius"]).and_then(value_f32) {
        profile.sky.moon_radius = moon_radius.clamp(1.0, 64.0);
        applied += 1;
    }
    if let Some(mesh) = value_path(metadata, &["sky", "mesh"]).and_then(value_string) {
        profile.sky.mesh = mesh;
        applied += 1;
    }
    if let Some(definition_ref) =
        value_path(metadata, &["sky", "definition_ref"]).and_then(value_string)
    {
        profile.sky.definition_ref = definition_ref;
        applied += 1;
    }
    if let Some(render_options) =
        value_path(metadata, &["sky", "render_options"]).and_then(|value| {
            serde_json::from_value::<newengine_model_domain_api::MeshRenderOptions>(value.clone())
                .ok()
        })
    {
        profile.sky.render_options = render_options;
        applied += 1;
    }
    if let Some(cloud_dictionary) =
        value_path(metadata, &["sky", "cloud_dictionary"]).and_then(value_string)
    {
        profile.sky.cloud_dictionary = cloud_dictionary;
        applied += 1;
    }
    if let Some(cloud_profile) =
        value_path(metadata, &["sky", "cloud_profile"]).and_then(value_string)
    {
        profile.sky.cloud_profile = cloud_profile;
        applied += 1;
    }
    applied
}

pub fn apply_time_constants_from_ytyp(
    profile: &mut AuthoredWorldProfile,
    metadata: &serde_json::Value,
) -> usize {
    let mut applied = 0usize;
    if let Some(hours) =
        value_path(metadata, &["lighting", "day_night", "time_of_day_hours"]).and_then(value_f32)
    {
        profile.lighting.day_night.time_of_day_hours = hours.rem_euclid(24.0);
        applied += 1;
    }
    if let Some(day_len) =
        value_path(metadata, &["lighting", "day_night", "day_length_seconds"]).and_then(value_f32)
    {
        profile.lighting.day_night.day_length_seconds = day_len.max(1.0);
        applied += 1;
    }
    if let Some(latitude) =
        value_path(metadata, &["lighting", "day_night", "latitude_degrees"]).and_then(value_f32)
    {
        profile.lighting.day_night.latitude_degrees = latitude.clamp(-89.0, 89.0);
        applied += 1;
    }
    if let Some(axial_tilt) =
        value_path(metadata, &["lighting", "day_night", "axial_tilt_degrees"]).and_then(value_f32)
    {
        profile.lighting.day_night.axial_tilt_degrees = axial_tilt.clamp(-45.0, 45.0);
        applied += 1;
    }
    applied
}
