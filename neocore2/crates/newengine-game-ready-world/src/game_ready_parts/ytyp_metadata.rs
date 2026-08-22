use super::*;

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
    value.as_f64().map(|v| v as f32).filter(|v| v.is_finite())
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

pub(super) fn apply_player_model_from_ytyp(
    profile: &mut GameReadyMapProfile,
    metadata: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let player_node = value_path(metadata, &["player"]);
    let Some(model) = player_node
        .and_then(|player| player.get("model"))
        .filter(|model| model.is_object())
        .or_else(|| {
            player_node.filter(|player| {
                player.get("source").is_some()
                    || player.get("texture_dictionary").is_some()
                    || player.get("metadata").is_some()
                    || player
                        .get("model")
                        .and_then(|value| value.as_str())
                        .is_some()
            })
        })
        .or_else(|| value_path(metadata, &["model"]))
        .or_else(|| player_node.and_then(|player| player.get("model")))
    else {
        return 0;
    };
    let mut applied = 0usize;
    let source = value_path(model, &["source"])
        .or_else(|| value_path(model, &["model"]))
        .and_then(value_string)
        .or_else(|| value_string(model));
    if let Some(source) = source {
        profile.player.model.source = source;
        profile.player.model.enabled = true;
        applied += 1;
    }
    if let Some(properties_ref) = value_path(model, &["properties_ref"])
        .or_else(|| value_path(model, &["descriptor_ref"]))
        .or_else(|| value_path(model, &["ytyp_ref"]))
        .and_then(value_string)
    {
        profile.player.model.properties_ref = Some(properties_ref);
        applied += 1;
    }
    if applied > 0 && profile.player.model.properties_ref.is_none() {
        let normalized_ref = definition_ref.trim().replace('\\', "/");
        if !normalized_ref.is_empty() {
            profile.player.model.properties_ref = Some(normalized_ref);
            applied += 1;
        }
    }
    if let Some(texture_dictionary) = value_path(model, &["texture_dictionary"])
        .or_else(|| value_path(model, &["textures"]))
        .and_then(value_string)
    {
        profile.player.model.texture_dictionary = Some(texture_dictionary);
        applied += 1;
    }
    if let Some(skeleton) = value_path(model, &["skeleton"])
        .or_else(|| value_path(model, &["metadata"]))
        .or_else(|| value_path(model, &["skeleton_ref"]))
        .and_then(value_string)
    {
        profile.player.model.skeleton = Some(skeleton);
        applied += 1;
    }
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
    if let Some(reference) = value_path(model, &["jump_animation"]).and_then(value_string) {
        profile.player.model.jump_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["fall_animation"]).and_then(value_string) {
        profile.player.model.fall_animation = Some(reference);
        applied += 1;
    }
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
    if applied > 0 {
        newengine_ulog_api::ulog::info!(
            "game-ready ytyp metadata: player model descriptor source='{}' properties_ref={:?} policy='.ytyp connects model source to material bindings'",
            profile.player.model.source,
            profile.player.model.properties_ref
        );
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

pub(super) fn game_ready_metadata_namespace(
    entry: &serde_json::Value,
) -> Option<&serde_json::Value> {
    // engine.assets.definitions returns arbitrary metadata as a source-of-knowledge
    // envelope: { arbitrary_metadata: { metadata: { ns: ... }, namespaces: { ns: ... } } }.
    // Older probes may still expose metadata directly. Accept both shapes, but
    // keep the namespace name fully data-authored; GameReady does not own .ytyp.
    entry
        .get("arbitrary_metadata")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get("newengine.game_ready"))
        .or_else(|| {
            entry
                .get("arbitrary_metadata")
                .and_then(|v| v.get("namespaces"))
                .and_then(|v| v.get("newengine.game_ready"))
        })
        .or_else(|| {
            entry
                .get("arbitrary_metadata")
                .and_then(|v| v.get("newengine.game_ready"))
        })
        .or_else(|| {
            entry
                .get("metadata")
                .and_then(|v| v.get("newengine.game_ready"))
        })
        .or_else(|| {
            entry
                .get("namespaces")
                .and_then(|v| v.get("newengine.game_ready"))
        })
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

pub(crate) fn apply_game_ready_ytyp_metadata(profile: &mut GameReadyMapProfile) {
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
        if metadata.is_null() {
            newengine_ulog_api::ulog::debug!(
                "game-ready ytyp metadata: definition_ref='{}' has no newengine.game_ready namespace; graph-only definition",
                definition_ref
            );
        }
        let applied = apply_render_options_from_ytyp(profile, &entry, definition_ref)
            + apply_material_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_texture_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_player_model_from_ytyp(profile, metadata, definition_ref)
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
