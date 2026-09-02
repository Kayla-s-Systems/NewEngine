fn camera_definition_namespace(entry: &serde_json::Value) -> Option<&serde_json::Value> {
    metadata_namespace(entry, "newengine.camera")
}

fn camera_required_value<'a>(
    root: &'a serde_json::Value,
    path: &[&str],
    definition_ref: &str,
) -> Result<&'a serde_json::Value, String> {
    value_path(root, path).ok_or_else(|| {
        format!(
            "camera definition missing field ref={} path={}",
            definition_ref,
            path.join(".")
        )
    })
}

fn camera_required_f32(
    root: &serde_json::Value,
    path: &[&str],
    definition_ref: &str,
) -> Result<f32, String> {
    let value = camera_required_value(root, path, definition_ref)?;
    value_f32(value).ok_or_else(|| {
        format!(
            "camera definition field must be finite number ref={} path={} value={}",
            definition_ref,
            path.join("."),
            value
        )
    })
}

fn camera_required_string(
    root: &serde_json::Value,
    path: &[&str],
    definition_ref: &str,
) -> Result<String, String> {
    let value = camera_required_value(root, path, definition_ref)?;
    value_string(value).ok_or_else(|| {
        format!(
            "camera definition field must be non-empty string ref={} path={} value={}",
            definition_ref,
            path.join("."),
            value
        )
    })
}

fn camera_required_bool(
    root: &serde_json::Value,
    path: &[&str],
    definition_ref: &str,
) -> Result<bool, String> {
    let value = camera_required_value(root, path, definition_ref)?;
    value_bool(value).ok_or_else(|| {
        format!(
            "camera definition field must be bool ref={} path={} value={}",
            definition_ref,
            path.join("."),
            value
        )
    })
}

fn camera_required_vec3(
    root: &serde_json::Value,
    path: &[&str],
    definition_ref: &str,
) -> Result<Vec3, String> {
    let value = camera_required_value(root, path, definition_ref)?;
    let values = if let Some(values) = value.as_array() {
        values.iter().map(value_f32).collect::<Option<Vec<_>>>()
    } else if let Some(text) = value.as_str() {
        text.split(',')
            .map(|atom| atom.trim().parse::<f32>().ok())
            .collect::<Option<Vec<_>>>()
    } else {
        None
    }
    .ok_or_else(|| {
        format!(
            "camera definition field must be vec3 ref={} path={} value={}",
            definition_ref,
            path.join("."),
            value
        )
    })?;
    if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
        return Err(format!(
            "camera definition field must contain 3 finite values ref={} path={} value={}",
            definition_ref,
            path.join("."),
            value
        ));
    }
    Ok(Vec3::new(values[0], values[1], values[2]))
}

fn hydrate_camera_definition(
    profile: &mut AuthoredWorldProfile,
    entry: &serde_json::Value,
    definition_ref: &str,
) -> Result<usize, String> {
    let namespace = camera_definition_namespace(entry).ok_or_else(|| {
        format!(
            "camera definition has no newengine.camera namespace ref={}",
            definition_ref
        )
    })?;
    let camera = value_path(namespace, &["camera"]).unwrap_or(namespace);
    let schema = value_path(camera, &["schema"])
        .and_then(value_string)
        .unwrap_or_default();
    if schema != "newengine.camera.definition.v3" {
        return Err(format!(
            "camera definition schema mismatch ref={} actual={} expected=newengine.camera.definition.v3",
            definition_ref, schema
        ));
    }
    let role = camera_required_string(camera, &["role"], definition_ref)?;
    if !role.eq_ignore_ascii_case("player") {
        return Err(format!(
            "camera definition role mismatch ref={} actual={} expected=player",
            definition_ref, role
        ));
    }
    if !camera_required_bool(camera, &["active"], definition_ref)? {
        return Err(format!(
            "player camera definition must declare active=true ref={}",
            definition_ref
        ));
    }
    let target = camera_required_string(camera, &["target"], definition_ref)?;
    if !target.eq_ignore_ascii_case("player") {
        return Err(format!(
            "player camera definition target mismatch ref={} actual={} expected=player",
            definition_ref, target
        ));
    }

    let initial_view = camera_required_string(camera, &["initial_view"], definition_ref)?;
    let initial_view = match initial_view.to_ascii_lowercase().as_str() {
        "first_person" | "firstperson" => {
            newengine_engine_runtime::gameplay::PlayerCameraViewMode::FirstPerson
        }
        "third_person_follow" | "thirdpersonfollow" | "follow" => {
            newengine_engine_runtime::gameplay::PlayerCameraViewMode::ThirdPersonFollow
        }
        "third_person_aim" | "thirdpersonaim" | "aim" => {
            newengine_engine_runtime::gameplay::PlayerCameraViewMode::ThirdPersonAim
        }
        "third_person_orbit" | "thirdpersonorbit" | "orbit" => {
            newengine_engine_runtime::gameplay::PlayerCameraViewMode::ThirdPersonOrbit
        }
        _ => {
            return Err(format!(
                "camera definition initial_view is invalid ref={} value={} expected=first_person|third_person_follow|third_person_aim|third_person_orbit",
                definition_ref, initial_view
            ));
        }
    };

    let c = &mut profile.gameplay.camera;
    c.definition_ref = definition_ref.to_owned();
    c.initial_view = initial_view;

    c.first_person_fov_y_radians =
        camera_required_f32(camera, &["first_person", "fov_y_degrees"], definition_ref)?
            .to_radians();
    c.first_person_ads_fov_y_radians = camera_required_f32(
        camera,
        &["first_person", "ads_fov_y_degrees"],
        definition_ref,
    )?
    .to_radians();
    c.first_person_near = camera_required_f32(camera, &["first_person", "near"], definition_ref)?;
    c.first_person_forward_clearance = camera_required_f32(
        camera,
        &["first_person", "forward_clearance"],
        definition_ref,
    )?;
    c.first_person_body_yaw_limit_radians = camera_required_f32(
        camera,
        &["first_person", "body_yaw_limit_degrees"],
        definition_ref,
    )?
    .to_radians();
    c.first_person_down_pitch_limit_radians = camera_required_f32(
        camera,
        &["first_person", "down_pitch_limit_degrees"],
        definition_ref,
    )?
    .to_radians();
    c.first_person_collision_enabled = camera_required_bool(
        camera,
        &["first_person", "collision_enabled"],
        definition_ref,
    )?;
    c.first_person_collision_probe_radius = camera_required_f32(
        camera,
        &["first_person", "collision_probe_radius"],
        definition_ref,
    )?;
    c.first_person_collision_padding = camera_required_f32(
        camera,
        &["first_person", "collision_padding"],
        definition_ref,
    )?;
    c.first_person_grounded_eye_deadband_m = camera_required_f32(
        camera,
        &["first_person", "grounded_eye_deadband_m"],
        definition_ref,
    )?;
    c.first_person_grounded_eye_time_constant_seconds = camera_required_f32(
        camera,
        &["first_person", "grounded_eye_time_constant_seconds"],
        definition_ref,
    )?;
    c.first_person_camera_recoil_share = camera_required_f32(
        camera,
        &["first_person", "camera_recoil_share"],
        definition_ref,
    )?;
    c.first_person_aim_response_hz =
        camera_required_f32(camera, &["first_person", "aim_response_hz"], definition_ref)?;
    c.hide_local_model_in_first_person = camera_required_bool(
        camera,
        &["first_person", "hide_local_model"],
        definition_ref,
    )?;

    c.near_clip_enabled = camera_required_bool(camera, &["near_clip", "enabled"], definition_ref)?;
    c.near_clip_first_person_max_distance = camera_required_f32(
        camera,
        &["near_clip", "first_person_max_distance"],
        definition_ref,
    )?;
    c.near_clip_third_person_min_distance = camera_required_f32(
        camera,
        &["near_clip", "third_person_min_distance"],
        definition_ref,
    )?;
    c.near_clip_third_person_max_distance = camera_required_f32(
        camera,
        &["near_clip", "third_person_max_distance"],
        definition_ref,
    )?;
    c.near_clip_pull_in_distance =
        camera_required_f32(camera, &["near_clip", "pull_in_distance"], definition_ref)?;
    c.near_clip_probe_radius =
        camera_required_f32(camera, &["near_clip", "probe_radius"], definition_ref)?;
    c.near_clip_release_time_seconds = camera_required_f32(
        camera,
        &["near_clip", "release_time_seconds"],
        definition_ref,
    )?;
    c.near_clip_hysteresis_m =
        camera_required_f32(camera, &["near_clip", "hysteresis_m"], definition_ref)?;

    c.third_person_collision_enabled = camera_required_bool(
        camera,
        &["third_person", "collision_enabled"],
        definition_ref,
    )?;
    c.third_person_collision_probe_radius = camera_required_f32(
        camera,
        &["third_person", "collision_probe_radius"],
        definition_ref,
    )?;
    c.third_person_collision_padding = camera_required_f32(
        camera,
        &["third_person", "collision_padding"],
        definition_ref,
    )?;
    c.third_person_collision_min_distance = camera_required_f32(
        camera,
        &["third_person", "collision_min_distance"],
        definition_ref,
    )?;
    c.third_person_collision_release_frequency_hz = camera_required_f32(
        camera,
        &["third_person", "collision_release_frequency_hz"],
        definition_ref,
    )?;
    c.third_person_collision_release_damping_ratio = camera_required_f32(
        camera,
        &["third_person", "collision_release_damping_ratio"],
        definition_ref,
    )?;
    c.third_person_collision_distance_hysteresis = camera_required_f32(
        camera,
        &["third_person", "collision_distance_hysteresis_m"],
        definition_ref,
    )?;

    c.third_person_look_at_collision_blend = camera_required_f32(
        camera,
        &["third_person", "look_at", "collision_blend"],
        definition_ref,
    )?;
    c.third_person_look_at_response_hz = camera_required_f32(
        camera,
        &["third_person", "look_at", "response_hz"],
        definition_ref,
    )?;
    c.third_person_look_at_max_error_fov_fraction = camera_required_f32(
        camera,
        &["third_person", "look_at", "max_error_fov_fraction"],
        definition_ref,
    )?;
    c.third_person_catch_up_enabled = camera_required_bool(
        camera,
        &["third_person", "catch_up", "enabled"],
        definition_ref,
    )?;
    c.third_person_catch_up_frequency_hz = camera_required_f32(
        camera,
        &["third_person", "catch_up", "frequency_hz"],
        definition_ref,
    )?;
    c.third_person_catch_up_damping_ratio = camera_required_f32(
        camera,
        &["third_person", "catch_up", "damping_ratio"],
        definition_ref,
    )?;
    c.third_person_catch_up_max_distance_m = camera_required_f32(
        camera,
        &["third_person", "catch_up", "max_distance_m"],
        definition_ref,
    )?;
    c.third_person_catch_up_settle_distance_m = camera_required_f32(
        camera,
        &["third_person", "catch_up", "settle_distance_m"],
        definition_ref,
    )?;

    c.third_person_follow_fov_y_radians = camera_required_f32(
        camera,
        &["third_person", "follow", "fov_y_degrees"],
        definition_ref,
    )?
    .to_radians();
    c.third_person_follow_offset_ls = camera_required_vec3(
        camera,
        &["third_person", "follow", "offset"],
        definition_ref,
    )?;
    c.third_person_follow_focus_offset_ls = camera_required_vec3(
        camera,
        &["third_person", "follow", "focus_offset"],
        definition_ref,
    )?;
    c.third_person_follow_smooth_time = camera_required_f32(
        camera,
        &["third_person", "follow", "smooth_time"],
        definition_ref,
    )?;
    c.third_person_follow_max_speed = camera_required_f32(
        camera,
        &["third_person", "follow", "max_speed"],
        definition_ref,
    )?;
    c.third_person_follow_zoom_min = camera_required_f32(
        camera,
        &["third_person", "follow", "zoom_min"],
        definition_ref,
    )?;
    c.third_person_follow_zoom_max = camera_required_f32(
        camera,
        &["third_person", "follow", "zoom_max"],
        definition_ref,
    )?;

    c.third_person_aim_fov_y_radians = camera_required_f32(
        camera,
        &["third_person", "aim", "fov_y_degrees"],
        definition_ref,
    )?
    .to_radians();
    c.third_person_aim_offset_ls =
        camera_required_vec3(camera, &["third_person", "aim", "offset"], definition_ref)?;
    c.third_person_aim_focus_offset_ls = camera_required_vec3(
        camera,
        &["third_person", "aim", "focus_offset"],
        definition_ref,
    )?;
    c.third_person_aim_smooth_time = camera_required_f32(
        camera,
        &["third_person", "aim", "smooth_time"],
        definition_ref,
    )?;
    c.third_person_aim_max_speed = camera_required_f32(
        camera,
        &["third_person", "aim", "max_speed"],
        definition_ref,
    )?;
    c.third_person_aim_zoom_min =
        camera_required_f32(camera, &["third_person", "aim", "zoom_min"], definition_ref)?;
    c.third_person_aim_zoom_max =
        camera_required_f32(camera, &["third_person", "aim", "zoom_max"], definition_ref)?;

    c.third_person_orbit_fov_y_radians = camera_required_f32(
        camera,
        &["third_person", "orbit", "fov_y_degrees"],
        definition_ref,
    )?
    .to_radians();
    c.third_person_orbit_offset_ls =
        camera_required_vec3(camera, &["third_person", "orbit", "offset"], definition_ref)?;
    c.third_person_orbit_focus_offset_ls = camera_required_vec3(
        camera,
        &["third_person", "orbit", "focus_offset"],
        definition_ref,
    )?;
    c.third_person_orbit_smooth_time = camera_required_f32(
        camera,
        &["third_person", "orbit", "smooth_time"],
        definition_ref,
    )?;
    c.third_person_orbit_max_speed = camera_required_f32(
        camera,
        &["third_person", "orbit", "max_speed"],
        definition_ref,
    )?;
    c.third_person_orbit_zoom_min = camera_required_f32(
        camera,
        &["third_person", "orbit", "zoom_min"],
        definition_ref,
    )?;
    c.third_person_orbit_zoom_max = camera_required_f32(
        camera,
        &["third_person", "orbit", "zoom_max"],
        definition_ref,
    )?;
    c.third_person_orbit_look_sensitivity_radians_per_pixel = camera_required_f32(
        camera,
        &[
            "third_person",
            "orbit",
            "look_sensitivity_radians_per_pixel",
        ],
        definition_ref,
    )?;
    c.third_person_orbit_pitch_min_radians = camera_required_f32(
        camera,
        &["third_person", "orbit", "pitch_min_degrees"],
        definition_ref,
    )?
    .to_radians();
    c.third_person_orbit_pitch_max_radians = camera_required_f32(
        camera,
        &["third_person", "orbit", "pitch_max_degrees"],
        definition_ref,
    )?
    .to_radians();
    if c.third_person_orbit_pitch_min_radians >= c.third_person_orbit_pitch_max_radians {
        return Err(format!(
            "camera definition orbit pitch range is invalid ref={} min_deg={} max_deg={}",
            definition_ref,
            c.third_person_orbit_pitch_min_radians.to_degrees(),
            c.third_person_orbit_pitch_max_radians.to_degrees(),
        ));
    }
    c.orbit_drag_zoom_exponent_per_pixel = camera_required_f32(
        camera,
        &["third_person", "orbit", "drag_zoom_exponent_per_pixel"],
        definition_ref,
    )?;

    c.zoom_wheel_exponent_per_step =
        camera_required_f32(camera, &["zoom", "wheel_exponent_per_step"], definition_ref)?;
    c.zoom_smooth_time_seconds =
        camera_required_f32(camera, &["zoom", "smooth_time_seconds"], definition_ref)?;

    for (path, value) in [
        (
            "first_person.grounded_eye_time_constant_seconds",
            c.first_person_grounded_eye_time_constant_seconds,
        ),
        (
            "first_person.aim_response_hz",
            c.first_person_aim_response_hz,
        ),
        (
            "third_person.collision_release_frequency_hz",
            c.third_person_collision_release_frequency_hz,
        ),
        (
            "third_person.collision_release_damping_ratio",
            c.third_person_collision_release_damping_ratio,
        ),
        (
            "near_clip.first_person_max_distance",
            c.near_clip_first_person_max_distance,
        ),
        (
            "near_clip.third_person_min_distance",
            c.near_clip_third_person_min_distance,
        ),
        (
            "near_clip.third_person_max_distance",
            c.near_clip_third_person_max_distance,
        ),
        (
            "near_clip.release_time_seconds",
            c.near_clip_release_time_seconds,
        ),
        (
            "third_person.look_at.response_hz",
            c.third_person_look_at_response_hz,
        ),
        (
            "third_person.catch_up.frequency_hz",
            c.third_person_catch_up_frequency_hz,
        ),
        (
            "third_person.catch_up.damping_ratio",
            c.third_person_catch_up_damping_ratio,
        ),
        (
            "third_person.catch_up.max_distance_m",
            c.third_person_catch_up_max_distance_m,
        ),
        (
            "third_person.orbit.look_sensitivity_radians_per_pixel",
            c.third_person_orbit_look_sensitivity_radians_per_pixel,
        ),
        ("zoom.smooth_time_seconds", c.zoom_smooth_time_seconds),
    ] {
        if value <= 0.0 {
            return Err(format!(
                "camera definition field must be > 0 ref={} path={} value={}",
                definition_ref, path, value
            ));
        }
    }
    for (path, value) in [
        (
            "first_person.grounded_eye_deadband_m",
            c.first_person_grounded_eye_deadband_m,
        ),
        (
            "first_person.camera_recoil_share",
            c.first_person_camera_recoil_share,
        ),
        (
            "third_person.collision_distance_hysteresis_m",
            c.third_person_collision_distance_hysteresis,
        ),
        ("near_clip.pull_in_distance", c.near_clip_pull_in_distance),
        ("near_clip.probe_radius", c.near_clip_probe_radius),
        ("near_clip.hysteresis_m", c.near_clip_hysteresis_m),
        (
            "third_person.catch_up.settle_distance_m",
            c.third_person_catch_up_settle_distance_m,
        ),
    ] {
        if value < 0.0 {
            return Err(format!(
                "camera definition field must be >= 0 ref={} path={} value={}",
                definition_ref, path, value
            ));
        }
    }

    if c.near_clip_first_person_max_distance < c.first_person_near {
        return Err(format!(
            "camera definition near clip FPP range is invalid ref={} min={} max={}",
            definition_ref, c.first_person_near, c.near_clip_first_person_max_distance
        ));
    }
    if c.near_clip_third_person_max_distance < c.near_clip_third_person_min_distance {
        return Err(format!(
            "camera definition near clip TPP range is invalid ref={} min={} max={}",
            definition_ref,
            c.near_clip_third_person_min_distance,
            c.near_clip_third_person_max_distance
        ));
    }
    if !(0.0..=1.0).contains(&c.third_person_look_at_collision_blend) {
        return Err(format!(
            "camera definition look_at collision_blend must be within [0,1] ref={} value={}",
            definition_ref, c.third_person_look_at_collision_blend
        ));
    }
    if !(0.0..=1.0).contains(&c.third_person_look_at_max_error_fov_fraction) {
        return Err(format!(
            "camera definition look_at max_error_fov_fraction must be within [0,1] ref={} value={}",
            definition_ref, c.third_person_look_at_max_error_fov_fraction
        ));
    }
    if c.third_person_catch_up_settle_distance_m > c.third_person_catch_up_max_distance_m {
        return Err(format!(
            "camera definition catch_up settle distance exceeds max distance ref={} settle={} max={}",
            definition_ref,
            c.third_person_catch_up_settle_distance_m,
            c.third_person_catch_up_max_distance_m
        ));
    }

    c.gameplay_blend_in_seconds =
        camera_required_f32(camera, &["blend", "in_seconds"], definition_ref)?;
    c.gameplay_blend_out_seconds =
        camera_required_f32(camera, &["blend", "out_seconds"], definition_ref)?;
    c.gameplay_blend_lock_input =
        camera_required_bool(camera, &["blend", "lock_input"], definition_ref)?;

    Ok(67)
}

pub(crate) fn apply_required_camera_definition(
    profile: &mut AuthoredWorldProfile,
) -> Result<(), String> {
    if !profile.gameplay.camera.declared {
        return Err("player camera is not declared by the authored map".to_owned());
    }
    let definition_ref = profile.gameplay.camera.definition_ref.trim().to_owned();
    if definition_ref.is_empty() {
        return Err(format!(
            "authored player camera '{}' has empty definition_ref",
            profile.gameplay.camera.instance_id
        ));
    }
    let entry = load_game_ready_definition_entry(&definition_ref).ok_or_else(|| {
        format!(
            "camera definition unavailable through engine.assets.definitions ref={}",
            definition_ref
        )
    })?;
    let applied = hydrate_camera_definition(profile, &entry, &definition_ref)?;
    newengine_ulog_api::ulog::info!(
        "game-ready camera definition hydrated camera='{}' ref='{}' fields={} source='engine.assets.definitions/newengine.camera' policy='no engine camera fallback'",
        profile.gameplay.camera.instance_id,
        definition_ref,
        applied
    );
    Ok(())
}

pub(super) fn game_ready_metadata_namespace(
    entry: &serde_json::Value,
) -> Option<&serde_json::Value> {
    metadata_namespace(entry, "newengine.game_ready")
}
