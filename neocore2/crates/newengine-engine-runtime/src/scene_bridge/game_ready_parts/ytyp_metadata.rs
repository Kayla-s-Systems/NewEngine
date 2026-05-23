/// GameReady metadata hydration from `.ytyp` Definition Entries.
///
/// `.ymap` is the launch/profile document. Runtime constants that belong
/// to authored game content are allowed to override the profile only when they
/// come from `engine.definitions` as `.ytyp@entry` metadata. This keeps the
/// chain visible:
///
/// `.ytyp -> .ydd -> .nemat -> .ytd`
///
/// and prevents local ad-hoc material JSON from becoming a second source of
/// truth.
#[inline]
fn value_path<'a>(mut value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

#[inline]
fn value_string(value: &serde_json::Value) -> Option<String> {
    value.as_str().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.replace('\\', "/"))
}

#[inline]
fn value_f32(value: &serde_json::Value) -> Option<f32> {
    value.as_f64().map(|v| v as f32).filter(|v| v.is_finite())
}

#[inline]
fn is_nemat_ref(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    let Some((file, entry)) = normalized.rsplit_once('@') else { return false; };
    !entry.trim().is_empty() && file.to_ascii_lowercase().ends_with(".nemat")
}

#[inline]
fn is_ytd_ref(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    let Some((file, entry)) = normalized.rsplit_once('@') else { return false; };
    !entry.trim().is_empty() && file.to_ascii_lowercase().ends_with(".ytd")
}

#[inline]
fn material_spec_mut<'a>(profile: &'a mut GameReadyMapProfile, key: &str) -> Option<&'a mut GameReadyMaterialSpec> {
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

fn apply_material_refs_from_ytyp(profile: &mut GameReadyMapProfile, metadata: &serde_json::Value, definition_ref: &str) -> usize {
    let Some(materials) = metadata.get("materials").and_then(|v| v.as_object()) else { return 0; };
    let mut applied = 0usize;
    for (key, value) in materials {
        let Some(reference) = value_string(value) else { continue; };
        if !is_nemat_ref(&reference) {
            log::warn!(
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
            log::debug!(
                "game-ready ytyp metadata: material key='{}' asset='{}' source='{}'",
                key,
                reference,
                definition_ref
            );
        }
    }
    applied
}

fn apply_ytd_constant(
    metadata: &serde_json::Value,
    path: &[&str],
    label: &str,
    definition_ref: &str,
) -> Option<String> {
    let reference = value_path(metadata, path).and_then(value_string)?;
    if !is_ytd_ref(&reference) {
        log::warn!(
            "game-ready ytyp metadata: rejected texture label='{}' ref='{}' definition_ref='{}' reason='expected .ytd@entry'",
            label,
            reference,
            definition_ref
        );
        return None;
    }
    Some(reference)
}

fn apply_texture_refs_from_ytyp(profile: &mut GameReadyMapProfile, metadata: &serde_json::Value, definition_ref: &str) -> usize {
    let mut applied = 0usize;
    if let Some(reference) = apply_ytd_constant(metadata, &["terrain", "surface", "forest_base_texture"], "terrain.surface.forest_base_texture", definition_ref) {
        profile.terrain.surface.forest_base_texture = reference;
        applied += 1;
    }
    if let Some(reference) = apply_ytd_constant(metadata, &["terrain", "surface", "sand_base_texture"], "terrain.surface.sand_base_texture", definition_ref) {
        profile.terrain.surface.sand_base_texture = reference;
        applied += 1;
    }
    if let Some(reference) = apply_ytd_constant(metadata, &["terrain", "surface", "rock_base_texture"], "terrain.surface.rock_base_texture", definition_ref) {
        profile.terrain.surface.rock_base_texture = reference;
        applied += 1;
    }
    if let Some(reference) = apply_ytd_constant(metadata, &["sky", "moon_texture"], "sky.moon_texture", definition_ref) {
        profile.sky.moon_texture = reference;
        applied += 1;
    }
    applied
}

fn apply_sky_constants_from_ytyp(profile: &mut GameReadyMapProfile, metadata: &serde_json::Value) -> usize {
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
    if let Some(cloud_dictionary) = value_path(metadata, &["sky", "cloud_dictionary"]).and_then(value_string) {
        profile.sky.cloud_dictionary = cloud_dictionary;
        applied += 1;
    }
    if let Some(cloud_profile) = value_path(metadata, &["sky", "cloud_profile"]).and_then(value_string) {
        profile.sky.cloud_profile = cloud_profile;
        applied += 1;
    }
    applied
}

fn apply_time_constants_from_ytyp(profile: &mut GameReadyMapProfile, metadata: &serde_json::Value) -> usize {
    let mut applied = 0usize;
    if let Some(hours) = value_path(metadata, &["lighting", "day_night", "time_of_day_hours"]).and_then(value_f32) {
        profile.lighting.day_night.time_of_day_hours = hours.rem_euclid(24.0);
        applied += 1;
    }
    if let Some(day_len) = value_path(metadata, &["lighting", "day_night", "day_length_seconds"]).and_then(value_f32) {
        profile.lighting.day_night.day_length_seconds = day_len.max(1.0);
        applied += 1;
    }
    if let Some(latitude) = value_path(metadata, &["lighting", "day_night", "latitude_degrees"]).and_then(value_f32) {
        profile.lighting.day_night.latitude_degrees = latitude.clamp(-89.0, 89.0);
        applied += 1;
    }
    if let Some(axial_tilt) = value_path(metadata, &["lighting", "day_night", "axial_tilt_degrees"]).and_then(value_f32) {
        profile.lighting.day_night.axial_tilt_degrees = axial_tilt.clamp(-45.0, 45.0);
        applied += 1;
    }
    applied
}

fn load_game_ready_definition_entry(definition_ref: &str) -> Option<serde_json::Value> {
    let payload = serde_json::to_vec(&serde_json::json!({ "definition_ref": definition_ref })).ok()?;
    match call_service_v1("engine.definitions", "definitions.entry_json_v1", &payload) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => Some(value),
            Err(e) => {
                log::warn!("game-ready ytyp metadata: engine.definitions returned invalid json ref='{}' err='{}'", definition_ref, e);
                None
            }
        },
        Err(e) => {
            log::warn!("game-ready ytyp metadata: engine.definitions unavailable ref='{}' err='{}'", definition_ref, e);
            None
        }
    }
}

fn game_ready_metadata_namespace(entry: &serde_json::Value) -> Option<&serde_json::Value> {
    // engine.definitions returns arbitrary metadata as a source-of-knowledge
    // envelope: { arbitrary_metadata: { metadata: { ns: ... }, namespaces: { ns: ... } } }.
    // Older probes may still expose metadata directly. Accept both shapes, but
    // keep the namespace name fully data-authored; GameReady does not own .ytyp.
    entry
        .get("arbitrary_metadata")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get("newengine.game_ready"))
        .or_else(|| entry.get("arbitrary_metadata").and_then(|v| v.get("namespaces")).and_then(|v| v.get("newengine.game_ready")))
        .or_else(|| entry.get("arbitrary_metadata").and_then(|v| v.get("newengine.game_ready")))
        .or_else(|| entry.get("metadata").and_then(|v| v.get("newengine.game_ready")))
        .or_else(|| entry.get("namespaces").and_then(|v| v.get("newengine.game_ready")))
}

fn apply_game_ready_ytyp_metadata(profile: &mut GameReadyMapProfile) {
    let definitions = profile.definitions.clone();
    if definitions.is_empty() {
        log::warn!(
            "game-ready ytyp metadata: scene profile declares no .ytyp Definition Entries; runtime will use sanitized profile defaults"
        );
        return;
    }

    let mut applied_total = 0usize;
    for spec in definitions {
        let definition_ref = spec.definition_ref.trim();
        let Some(entry) = load_game_ready_definition_entry(definition_ref) else { continue; };
        let null_metadata = serde_json::Value::Null;
        let metadata = game_ready_metadata_namespace(&entry).unwrap_or(&null_metadata);
        if metadata.is_null() {
            log::debug!(
                "game-ready ytyp metadata: definition_ref='{}' has no newengine.game_ready namespace; graph-only definition",
                definition_ref
            );
        }
        let applied = apply_material_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_texture_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_sky_constants_from_ytyp(profile, metadata)
            + apply_time_constants_from_ytyp(profile, metadata);
        applied_total += applied;
        log::info!(
            "game-ready ytyp metadata: consumed definition_ref='{}' applied_constants={} policy='metadata constants from engine.definitions; no runtime json material source'",
            definition_ref,
            applied
        );
    }

    log::info!(
        "game-ready ytyp metadata: completed definitions={} applied_constants={} chain='.ytyp -> .ydd -> .nemat -> .ytd'",
        profile.definitions.len(),
        applied_total
    );
}

fn resolve_game_ready_asset_graph(root_ref: &str) -> Option<newengine_model_domain_api::ResolvedAssetGraphV2> {
    let payload = serde_json::to_vec(&serde_json::json!({ "root_ref": root_ref })).ok()?;
    match call_service_v1(
        newengine_model_domain_api::ENGINE_ASSET_GRAPH_SERVICE_ID,
        newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1,
        &payload,
    ) {
        Ok(bytes) => match serde_json::from_slice::<newengine_model_domain_api::ResolvedAssetGraphV2>(&bytes) {
            Ok(graph) => Some(graph),
            Err(e) => {
                log::warn!("asset_graph.resolve_v1: invalid json graph root_ref='{}' err='{}'", root_ref, e);
                None
            }
        },
        Err(e) => {
            log::warn!("asset_graph.resolve_v1: gateway call failed root_ref='{}' err='{}'", root_ref, e);
            None
        }
    }
}
