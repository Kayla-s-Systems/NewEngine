use super::*;
use newengine_game_data::{GameData, PlayerMotionResponseData};

#[path = "ytyp_metadata/player.rs"]
mod player;
use player::apply_player_model_from_ytyp;
#[cfg(test)]
use player::player_joint_rotation_weights;

include!("ytyp_metadata/camera.rs");

/// GameReady metadata hydration from `.ytyp` Definition Entries.
///
/// `.ymap` is the launch/profile document. Runtime constants that belong
/// to authored game content are allowed to override the profile only when they
/// come from `engine.assets.definitions` as `.ytyp@entry` metadata. This keeps the
/// chain visible:
///
/// `.ytyp -> .ydd -> .nemat -> .ytd`
///
/// and prevents local ad-hoc material JSON from becoming a second source of
/// truth.
#[inline]
pub(super) fn value_path<'a>(
    mut value: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

#[inline]
pub(super) fn value_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('\\', "/"))
}

#[inline]
pub(super) fn value_f32(value: &serde_json::Value) -> Option<f32> {
    value
        .as_f64()
        .map(|v| v as f32)
        .or_else(|| value.as_str().and_then(|v| v.trim().parse::<f32>().ok()))
        .filter(|v| v.is_finite())
}

#[inline]
pub(super) fn value_bool(value: &serde_json::Value) -> Option<bool> {
    value.as_bool().or_else(|| {
        let raw = value.as_str()?.trim().to_ascii_lowercase();
        match raw.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

#[inline]
pub(super) fn is_nemat_ref(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    newengine_assets::require_asset_reference_extension(&normalized, &["nemat"], true).is_ok()
}

#[inline]
pub(super) fn is_ytd_ref(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    newengine_assets::require_asset_reference_extension(&normalized, &["ytd"], true).is_ok()
}

#[inline]
pub(super) fn material_spec_mut<'a>(
    profile: &'a mut GameReadyMapProfile,
    key: &str,
) -> Option<&'a mut GameReadyMaterialSpec> {
    match key {
        "terrain" => Some(&mut profile.materials.terrain),
        "sky" | "skydome" => Some(&mut profile.materials.sky),
        "sun" | "sun_disk" => Some(&mut profile.materials.sun),
        "moon" | "moon_disk" => Some(&mut profile.materials.moon),
        "tree_bark" | "bark" => Some(&mut profile.materials.tree_bark),
        "tree_leaf" | "leaf" => Some(&mut profile.materials.tree_leaf),
        "tree_branch" | "branch" => Some(&mut profile.materials.tree_branch),
        _ => None,
    }
}

pub(super) fn apply_material_refs_from_ytyp(
    profile: &mut GameReadyMapProfile,
    metadata: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let Some(materials) = metadata.get("materials").and_then(|v| v.as_object()) else {
        return 0;
    };
    let mut applied = 0usize;
    for (key, value) in materials {
        let Some(reference) = value_string(value) else {
            continue;
        };
        if !is_nemat_ref(&reference) {
            newengine_ulog_api::ulog::warn!(
                "game-ready ytyp metadata: rejected material key='{}' ref='{}' definition_ref='{}' reason='expected .nemat@entry'",
                key,
                reference,
                definition_ref
            );
            continue;
        }
        if let Some(spec) = material_spec_mut(profile, key) {
            spec.asset = Some(reference.clone());
            applied += 1;
            newengine_ulog_api::ulog::debug!(
                "game-ready ytyp metadata: material key='{}' asset='{}' source='{}'",
                key,
                reference,
                definition_ref
            );
        }
    }
    applied
}

pub(super) fn apply_ytd_constant(
    metadata: &serde_json::Value,
    path: &[&str],
    label: &str,
    definition_ref: &str,
) -> Option<String> {
    let reference = value_path(metadata, path).and_then(value_string)?;
    if !is_ytd_ref(&reference) {
        newengine_ulog_api::ulog::warn!(
            "game-ready ytyp metadata: rejected texture label='{}' ref='{}' definition_ref='{}' reason='expected .ytd@entry'",
            label,
            reference,
            definition_ref
        );
        return None;
    }
    Some(reference)
}

pub(super) fn apply_texture_refs_from_ytyp(
    profile: &mut GameReadyMapProfile,
    metadata: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let mut applied = 0usize;
    if let Some(reference) = apply_ytd_constant(
        metadata,
        &["terrain", "surface", "forest_base_texture"],
        "terrain.surface.forest_base_texture",
        definition_ref,
    ) {
        profile.terrain.surface.forest_base_texture = reference;
        applied += 1;
    }
    if let Some(reference) = apply_ytd_constant(
        metadata,
        &["terrain", "surface", "sand_base_texture"],
        "terrain.surface.sand_base_texture",
        definition_ref,
    ) {
        profile.terrain.surface.sand_base_texture = reference;
        applied += 1;
    }
    if let Some(reference) = apply_ytd_constant(
        metadata,
        &["terrain", "surface", "rock_base_texture"],
        "terrain.surface.rock_base_texture",
        definition_ref,
    ) {
        profile.terrain.surface.rock_base_texture = reference;
        applied += 1;
    }
    if let Some(reference) = apply_ytd_constant(
        metadata,
        &["sky", "moon_texture"],
        "sky.moon_texture",
        definition_ref,
    ) {
        profile.sky.moon_texture = reference;
        applied += 1;
    }
    applied
}

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
    profile: &mut GameReadyMapProfile,
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

pub(super) fn apply_gameplay_constants_from_ytyp(
    profile: &mut GameReadyMapProfile,
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

pub(super) fn apply_sky_constants_from_ytyp(
    profile: &mut GameReadyMapProfile,
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

pub(super) fn apply_time_constants_from_ytyp(
    profile: &mut GameReadyMapProfile,
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

pub(super) fn load_game_ready_definition_entry(definition_ref: &str) -> Option<serde_json::Value> {
    let payload =
        serde_json::to_vec(&serde_json::json!({ "definition_ref": definition_ref })).ok()?;
    match call_service_v1_optional(
        newengine_assets::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        newengine_assets::definitions_method::ENTRY_JSON_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => Some(value),
            Err(e) => {
                newengine_ulog_api::ulog::warn!("game-ready ytyp metadata: engine.assets.definitions returned invalid json ref='{}' err='{}'", definition_ref, e);
                None
            }
        },
        Ok(None) => {
            newengine_ulog_api::ulog::debug!("game-ready ytyp metadata: engine.assets.definitions route absent ref='{}'; metadata hydration skipped", definition_ref);
            None
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready ytyp metadata: engine.assets.definitions call failed ref='{}' err='{}'",
                definition_ref,
                e
            );
            None
        }
    }
}

fn metadata_namespace<'a>(
    entry: &'a serde_json::Value,
    namespace: &str,
) -> Option<&'a serde_json::Value> {
    entry
        .get("arbitrary_metadata")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get(namespace))
        .or_else(|| {
            entry
                .get("arbitrary_metadata")
                .and_then(|v| v.get("namespaces"))
                .and_then(|v| v.get(namespace))
        })
        .or_else(|| {
            entry
                .get("arbitrary_metadata")
                .and_then(|v| v.get(namespace))
        })
        .or_else(|| entry.get("metadata").and_then(|v| v.get(namespace)))
        .or_else(|| entry.get("namespaces").and_then(|v| v.get(namespace)))
}

pub(super) fn load_character_model_assignment(
    definition_ref: &str,
) -> Result<newengine_engine_runtime::gameplay::PlayerModelAssignment, String> {
    let definition_ref = definition_ref.trim();
    if definition_ref.is_empty() || !definition_ref.to_ascii_lowercase().contains(".ytyp@") {
        return Err(
            "character definition_ref must be an authored .ytyp@entry reference".to_owned(),
        );
    }
    let entry = load_game_ready_definition_entry(definition_ref)
        .ok_or_else(|| format!("character definition unavailable ref='{definition_ref}'"))?;
    let metadata = metadata_namespace(&entry, "newengine.game_ready").ok_or_else(|| {
        format!("character definition has no newengine.game_ready namespace ref='{definition_ref}'")
    })?;
    player::character_model_assignment_from_ytyp_metadata(metadata, definition_ref).ok_or_else(
        || {
            format!(
                "character definition has no authored model/idle contract ref='{definition_ref}'"
            )
        },
    )
}

fn audio_metadata_namespace(entry: &serde_json::Value) -> Option<&serde_json::Value> {
    metadata_namespace(entry, "newengine.audio")
}

fn string_or_array(value: &serde_json::Value) -> Vec<String> {
    if let Some(value) = value.as_str() {
        let value = value.trim().to_ascii_lowercase();
        return (!value.is_empty()).then_some(value).into_iter().collect();
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn acoustic_material_library_from_ytyp(
    metadata: &serde_json::Value,
) -> Option<newengine_audio_api::AcousticMaterialLibrary> {
    let library = metadata.get("acoustic_material_library")?;
    let raw_materials = library.get("material")?;
    let materials = raw_materials
        .as_array()
        .map(|values| values.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| vec![raw_materials]);
    let mut rules = Vec::new();
    for material in materials {
        let Some(material_id) = material.get("material_id").and_then(value_string) else {
            continue;
        };
        let Some(transmission_gain) = material.get("transmission_gain").and_then(value_f32) else {
            continue;
        };
        let reflection_gain = material
            .get("reflection_gain")
            .and_then(value_f32)
            .unwrap_or_else(|| {
                newengine_audio_api::AcousticMaterialProfile::default().reflection_gain
            });
        let Some(high_frequency_absorption) = material
            .get("high_frequency_absorption")
            .and_then(value_f32)
        else {
            continue;
        };
        let Some(low_pass_hz) = material.get("low_pass_hz").and_then(value_f32) else {
            continue;
        };
        let surface_matches = material
            .get("match")
            .map(string_or_array)
            .unwrap_or_default();
        if surface_matches.is_empty() {
            continue;
        }
        rules.push(newengine_audio_api::AcousticMaterialRule {
            material_id,
            surface_matches,
            profile: newengine_audio_api::AcousticMaterialProfile {
                transmission_gain,
                reflection_gain,
                high_frequency_absorption,
                low_pass_hz,
            },
        });
    }
    (!rules.is_empty()).then(|| newengine_audio_api::AcousticMaterialLibrary::new(rules))
}

fn merge_acoustic_material_library(
    target: &mut newengine_audio_api::AcousticMaterialLibrary,
    incoming: newengine_audio_api::AcousticMaterialLibrary,
) {
    for incoming_rule in incoming.rules {
        let incoming_matches = incoming_rule.surface_matches.clone();
        for rule in &mut target.rules {
            rule.surface_matches
                .retain(|pattern| !incoming_matches.iter().any(|value| value == pattern));
        }
        target.rules.retain(|rule| !rule.surface_matches.is_empty());
        target.rules.push(incoming_rule);
    }
    *target = target.clone().sanitized();
}

fn definition_render_options(
    entry: &serde_json::Value,
) -> Option<newengine_model_domain_api::MeshRenderOptions> {
    entry
        .get("model_explanation")
        .and_then(|value| value.get("render_options"))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn apply_sky_drawable_from_ytyp(
    profile: &mut GameReadyMapProfile,
    entry: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let Some(options) = definition_render_options(entry) else {
        return 0;
    };
    if !matches!(
        options.role,
        newengine_model_domain_api::MeshRenderRole::SkyBackground
            | newengine_model_domain_api::MeshRenderRole::CelestialBillboard
    ) {
        return 0;
    }
    let drawable = entry
        .get("model_explanation")
        .and_then(|value| value.get("drawable_ref"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(drawable) = drawable else {
        return 0;
    };
    profile.sky.mesh = drawable.replace('\\', "/");
    newengine_ulog_api::ulog::info!(
        "game-ready ytyp sky drawable: definition_ref='{}' mesh='{}' source='model_explanation.drawable_ref' policy='YTYP dependency graph owns skydome asset identity'",
        definition_ref,
        profile.sky.mesh
    );
    1
}

fn apply_render_options_from_ytyp(
    profile: &mut GameReadyMapProfile,
    entry: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let Some(options) = definition_render_options(entry) else {
        return 0;
    };
    let target = match options.role {
        newengine_model_domain_api::MeshRenderRole::TerrainPatch => {
            profile.terrain.render_options = options.clone();
            "terrain"
        }
        newengine_model_domain_api::MeshRenderRole::FoliageInstanced => {
            profile.foliage.render_options = options.clone();
            "foliage"
        }
        newengine_model_domain_api::MeshRenderRole::SkyBackground
        | newengine_model_domain_api::MeshRenderRole::CelestialBillboard => {
            profile.sky.render_options = options.clone();
            "sky"
        }
        newengine_model_domain_api::MeshRenderRole::CharacterBody => {
            profile.player.model.render_options = options.clone();
            "player"
        }
        _ => {
            return 0;
        }
    };
    newengine_ulog_api::ulog::info!("game-ready ytyp render policy: target='{}' definition_ref='{}' role={:?} shadow_policy={:?} source='engine.assets.definitions/.ytyp'", target, definition_ref, options.role, options.shadow_policy);
    1
}

pub(crate) fn apply_game_ready_ytyp_metadata(
    profile: &mut GameReadyMapProfile,
    game_data: &mut GameData,
) {
    let definitions = profile.definitions.clone();
    if definitions.is_empty() {
        newengine_ulog_api::ulog::warn!(
            "game-ready ytyp metadata: scene profile declares no .ytyp Definition Entries; runtime will use sanitized profile defaults"
        );
        return;
    }

    let mut applied_total = 0usize;
    for spec in definitions {
        let definition_ref = spec.definition_ref.trim();
        let Some(entry) = load_game_ready_definition_entry(definition_ref) else {
            continue;
        };
        let null_metadata = serde_json::Value::Null;
        let metadata = game_ready_metadata_namespace(&entry).unwrap_or(&null_metadata);
        let mut audio_applied = 0usize;
        if let Some(audio_metadata) = audio_metadata_namespace(&entry) {
            if let Some(library) = acoustic_material_library_from_ytyp(audio_metadata) {
                let count = library.rules.len();
                merge_acoustic_material_library(&mut profile.acoustic_materials, library);
                audio_applied = count;
                newengine_ulog_api::ulog::info!(
                    "game-ready ytyp audio metadata: definition_ref='{}' acoustic_material_rules={} total_rules={} policy='Shared baseline first; later definitions replace matching surface rules'",
                    definition_ref,
                    count,
                    profile.acoustic_materials.rules.len(),
                );
            }
        }
        if metadata.is_null() {
            newengine_ulog_api::ulog::debug!(
                "game-ready ytyp metadata: definition_ref='{}' has no newengine.game_ready namespace; graph-only definition",
                definition_ref
            );
        }
        let applied = audio_applied
            + apply_render_options_from_ytyp(profile, &entry, definition_ref)
            + apply_sky_drawable_from_ytyp(profile, &entry, definition_ref)
            + apply_material_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_texture_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_player_model_from_ytyp(profile, metadata, definition_ref)
            + apply_player_runtime_data_from_ytyp(profile, game_data, metadata)
            + apply_gameplay_constants_from_ytyp(profile, metadata)
            + apply_sky_constants_from_ytyp(profile, metadata)
            + apply_time_constants_from_ytyp(profile, metadata);
        applied_total += applied;
        newengine_ulog_api::ulog::info!(
            "game-ready ytyp metadata: consumed definition_ref='{}' applied_constants={} policy='metadata constants from engine.assets.definitions; no runtime json material source'",
            definition_ref,
            applied
        );
    }

    newengine_ulog_api::ulog::info!(
        "game-ready ytyp metadata: completed definitions={} applied_constants={} chain='.ytyp -> .ytyd -> .ydd -> .nemat -> .ytd'",
        profile.definitions.len(),
        applied_total
    );
}

pub(crate) fn resolve_game_ready_asset_graph(
    root_ref: &str,
) -> Option<newengine_model_domain_api::ResolvedAssetGraphV2> {
    let payload = serde_json::to_vec(&serde_json::json!({ "root_ref": root_ref })).ok()?;
    match call_service_v1_optional(
        newengine_model_domain_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
        newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => {
            match serde_json::from_slice::<newengine_model_domain_api::ResolvedAssetGraphV2>(&bytes)
            {
                Ok(graph) => Some(graph),
                Err(e) => {
                    newengine_ulog_api::ulog::warn!(
                        "assets.graph.resolve_v1: invalid json graph root_ref='{}' err='{}'",
                        root_ref,
                        e
                    );
                    None
                }
            }
        }
        Ok(None) => {
            newengine_ulog_api::ulog::debug!(
                "assets.graph.resolve_v1: route absent root_ref='{}'; graph hydration skipped",
                root_ref
            );
            None
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "assets.graph.resolve_v1: gateway call failed root_ref='{}' err='{}'",
                root_ref,
                e
            );
            None
        }
    }
}

#[cfg(test)]
#[path = "ytyp_metadata/tests.rs"]
mod tests;
