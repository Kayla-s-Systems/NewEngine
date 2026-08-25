use super::*;

use newengine_materials::{
    method as material_method, MaterialDescriptor as NeMaterialDescriptor,
    MaterialDescriptorLoadResponse as NeMaterialDescriptorLoadResponse,
    MaterialFlags as NeMaterialFlags, MaterialId as NeMaterialId,
    MaterialLoadRequest as NeMaterialLoadRequest, MaterialRegistry as NeMaterialRegistry,
    MaterialTextureBindings as NeMaterialTextureBindings, ENGINE_ASSETS_MATERIALS_SERVICE_ID,
};

use self::content::GameReadyMaterialSpec as NeGameReadyMaterialSpec;

#[inline]
pub(super) fn is_nemat_entry_ref(path: &str) -> bool {
    let value = path.trim().replace('\\', "/");
    newengine_assets::require_asset_reference_extension(&value, &["nemat"], true).is_ok()
}

#[inline]
pub(super) fn load_material_descriptor_asset(
    path: &str,
) -> Option<NeMaterialDescriptorLoadResponse> {
    if !is_nemat_entry_ref(path) {
        newengine_ulog_api::ulog::warn!(
            "game-ready material: rejected non-canonical material asset path='{}' expected='<logical-path>.nemat@entry' policy='ytyp->ydd->nemat->ytd' action='skip_asset'",
            path
        );
        return None;
    }

    let request = NeMaterialLoadRequest {
        logical_path: path.trim().replace('\\', "/"),
        selector: None,
    };
    let payload = match serde_json::to_vec(&request) {
        Ok(payload) => payload,
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready material: .nemat request encode failed path='{}' err='{}'",
                path,
                e
            );
            return None;
        }
    };
    let bytes = match call_service_v1_optional(
        ENGINE_ASSETS_MATERIALS_SERVICE_ID,
        material_method::LOAD_DESCRIPTOR_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            newengine_ulog_api::ulog::debug!(
                "game-ready material: .nemat descriptor route absent path='{}' gateway='engine.assets.materials' method='{}'",
                path,
                material_method::LOAD_DESCRIPTOR_V1
            );
            return None;
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready material: .nemat descriptor unavailable path='{}' gateway='engine.assets.materials' method='{}' err='{}'",
                path,
                material_method::LOAD_DESCRIPTOR_V1,
                e
            );
            return None;
        }
    };
    match serde_json::from_slice::<NeMaterialDescriptorLoadResponse>(&bytes) {
        Ok(response) => Some(response),
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready material: .nemat descriptor decode failed path='{}' err='{}'",
                path,
                e
            );
            None
        }
    }
}

#[inline]
pub(super) fn diagnostic_unresolved_material(
    name: &str,
    base_color: [f32; 4],
    emissive: [f32; 3],
    emissive_strength: f32,
    flags: NeMaterialFlags,
    spec: &NeGameReadyMaterialSpec,
) -> (NeMaterialDescriptor, NeMaterialTextureBindings) {
    newengine_ulog_api::ulog::warn!(
        "game-ready material: unresolved material name='{}' asset={:?} policy='runtime requires .nemat@entry from authored graph' action='register_diagnostic_material'",
        name,
        spec.asset
    );
    if spec.base_color_texture.is_some()
        || spec.normal_texture.is_some()
        || spec.roughness_texture.is_some()
        || spec.uv_scale != [1.0, 1.0]
        || spec.uv_offset != [0.0, 0.0]
    {
        newengine_ulog_api::ulog::debug!(
            "game-ready material: diagnostic material ignores inline texture slots name='{}' base={:?} normal={:?} roughness={:?} uv_scale={:?} uv_offset={:?} policy='inline JSON material slots are not runtime material sources; use .nemat@entry -> .ytd@entry'",
            name,
            spec.base_color_texture,
            spec.normal_texture,
            spec.roughness_texture,
            spec.uv_scale,
            spec.uv_offset
        );
    }

    let mut desc = NeMaterialDescriptor {
        base_color,
        emissive,
        emissive_strength,
        roughness: spec.roughness,
        normal_scale: spec.normal_scale,
        occlusion_strength: spec.occlusion_strength,
        flags,
        ..NeMaterialDescriptor::default()
    };
    desc.sanitize_in_place();
    (desc, NeMaterialTextureBindings::default())
}

#[inline]
fn material_registry_identity(source: &str, fallback_name: &str) -> String {
    let canonical = source.trim().replace('\\', "/");
    if canonical.is_empty() {
        fallback_name.to_owned()
    } else {
        canonical
    }
}

#[inline]
fn upsert_loaded_material(
    mats: &NeMaterialRegistry,
    fallback_name: &str,
    flags: NeMaterialFlags,
    mut response: NeMaterialDescriptorLoadResponse,
) -> NeMaterialId {
    response.descriptor.flags = response.descriptor.flags.union(flags);
    response.descriptor.sanitize_in_place();
    // `m00`, `m01`, ... are library-local selectors, not globally unique material names.
    // Registry identity must include the canonical NEMAT source or different libraries will
    // overwrite each other (for example Abby m00 and rifle m00).
    let registry_name = material_registry_identity(&response.source, fallback_name);
    let display_name = if response.name.trim().is_empty() {
        fallback_name
    } else {
        response.name.as_str()
    };
    newengine_ulog_api::ulog::debug!(
        "game-ready material: resolved .nemat material registry='{}' display='{}' source='{}' policy='canonical .nemat@entry identity'",
        registry_name,
        display_name,
        response.source
    );
    mats.upsert_named_with_textures(
        &registry_name,
        response.descriptor,
        response.textures.sanitized(),
    )
}

/// Strict authored material registration from an already-resolved canonical `.nemat@entry`.
/// Used by runtime model parts whose YDD directly carries its material dependency.
#[inline]
pub(super) fn register_required_material_ref(
    mats: &NeMaterialRegistry,
    name: &str,
    flags: NeMaterialFlags,
    asset_path: &str,
) -> Result<NeMaterialId, String> {
    let asset_path = asset_path.trim();
    if asset_path.is_empty() {
        return Err(format!(
            "required material '{name}' has an empty asset reference"
        ));
    }
    let response = load_material_descriptor_asset(asset_path).ok_or_else(|| {
        format!(
            "required material descriptor unavailable name='{}' asset='{}' gateway='engine.assets.materials'",
            name, asset_path
        )
    })?;
    Ok(upsert_loaded_material(mats, name, flags, response))
}

/// Strict authored material registration used by visuals that must never persist with a
/// diagnostic/fallback surface. A temporarily unavailable material gateway is a deferred spawn,
/// not a valid black/white material.
#[inline]
pub(super) fn register_required_material(
    mats: &NeMaterialRegistry,
    name: &str,
    flags: NeMaterialFlags,
    spec: &NeGameReadyMaterialSpec,
) -> Result<NeMaterialId, String> {
    let asset_path = spec
        .asset
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| {
            format!(
                "required material '{}' has no canonical .nemat@entry asset",
                name
            )
        })?;
    let response = load_material_descriptor_asset(asset_path).ok_or_else(|| {
        format!(
            "required material descriptor unavailable name='{}' asset='{}' gateway='engine.assets.materials'",
            name, asset_path
        )
    })?;
    Ok(upsert_loaded_material(mats, name, flags, response))
}

/// Central game-ready material registration path.
///
/// Runtime materials are resolved only through `engine.assets.materials` from
/// `.nemat@entry` selectors. Hand-built texture slots are authoring diagnostics,
/// not runtime material sources. Missing/broken content receives an explicit
/// diagnostic material so the frame can still report what is wrong.
#[inline]
pub(super) fn register_material(
    mats: &NeMaterialRegistry,
    name: &str,
    base_color: [f32; 4],
    emissive: [f32; 3],
    emissive_strength: f32,
    flags: NeMaterialFlags,
    spec: &NeGameReadyMaterialSpec,
) -> NeMaterialId {
    if let Some(asset_path) = spec.asset.as_deref() {
        if let Some(response) = load_material_descriptor_asset(asset_path) {
            return upsert_loaded_material(mats, name, flags, response);
        }
    } else {
        newengine_ulog_api::ulog::warn!(
            "game-ready material: missing material asset for name='{}' expected='<logical-path>.nemat@entry' policy='no runtime json/ad-hoc material source'",
            name
        );
    }

    let (desc, textures) =
        diagnostic_unresolved_material(name, base_color, emissive, emissive_strength, flags, spec);
    mats.upsert_named_with_textures(name, desc, textures)
}

#[cfg(test)]
mod material_identity_tests {
    use super::material_registry_identity;

    #[test]
    fn nemat_registry_identity_is_source_scoped_not_short_selector_scoped() {
        assert_eq!(
            material_registry_identity("shared/materials/weapon_rifle.nemat@m00", "m00"),
            "shared/materials/weapon_rifle.nemat@m00"
        );
        assert_ne!(
            material_registry_identity("shared/materials/weapon_rifle.nemat@m00", "m00"),
            material_registry_identity("shared/materials/characters/abby.nemat@m00", "m00")
        );
    }
}
