use super::*;
use newengine_core::call_service_v1_optional;
use newengine_game_data::{GameData, PlayerMotionResponseData};
use newengine_math::Vec3;

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
pub fn value_path<'a>(
    mut value: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

#[inline]
pub fn value_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('\\', "/"))
}

#[inline]
pub fn value_f32(value: &serde_json::Value) -> Option<f32> {
    value
        .as_f64()
        .map(|v| v as f32)
        .or_else(|| value.as_str().and_then(|v| v.trim().parse::<f32>().ok()))
        .filter(|v| v.is_finite())
}

#[inline]
pub fn value_bool(value: &serde_json::Value) -> Option<bool> {
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
pub fn is_nemat_ref(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    newengine_assets::require_asset_reference_extension(&normalized, &["nemat"], true).is_ok()
}

#[inline]
pub fn is_ytd_ref(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    newengine_assets::require_asset_reference_extension(&normalized, &["ytd"], true).is_ok()
}

#[inline]
pub fn material_spec_mut<'a>(
    profile: &'a mut AuthoredWorldProfile,
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

pub fn apply_material_refs_from_ytyp(
    profile: &mut AuthoredWorldProfile,
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

pub fn apply_ytd_constant(
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

pub fn apply_texture_refs_from_ytyp(
    profile: &mut AuthoredWorldProfile,
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
