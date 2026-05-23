use newengine_materials::{
    method as material_method, MaterialDescriptor as NeMaterialDescriptor,
    MaterialDescriptorLoadResponse as NeMaterialDescriptorLoadResponse,
    MaterialFlags as NeMaterialFlags, MaterialId as NeMaterialId,
    MaterialLoadRequest as NeMaterialLoadRequest, MaterialRegistry as NeMaterialRegistry,
    MaterialTextureBindings as NeMaterialTextureBindings, ENGINE_ASSETS_MATERIALS_SERVICE_ID,
};

use self::content::GameReadyMaterialSpec as NeGameReadyMaterialSpec;

#[inline]
fn is_nemat_entry_ref(path: &str) -> bool {
    let value = path.trim().replace('\\', "/");
    newengine_assets::require_asset_reference_extension(&value, &["nemat"], true).is_ok()
}

#[inline]
fn load_material_descriptor_asset(path: &str) -> Option<NeMaterialDescriptorLoadResponse> {
    if !is_nemat_entry_ref(path) {
        log::warn!(
            "game-ready material: rejected non-canonical material asset path='{}' expected='<logical-path>.nemat@entry' policy='ytyp->ydd->nemat->ytd' action='skip_asset'",
            path
        );
        return None;
    }

    let request = NeMaterialLoadRequest { logical_path: path.trim().replace('\\', "/"), selector: None };
    let payload = match serde_json::to_vec(&request) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!("game-ready material: .nemat request encode failed path='{}' err='{}'", path, e);
            return None;
        }
    };
    let bytes = match call_service_v1(
        ENGINE_ASSETS_MATERIALS_SERVICE_ID,
        material_method::LOAD_DESCRIPTOR_V1,
        &payload,
    ) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::warn!(
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
            log::warn!("game-ready material: .nemat descriptor decode failed path='{}' err='{}'", path, e);
            None
        }
    }
}

#[inline]
fn diagnostic_unresolved_material(
    name: &str,
    base_color: [f32; 4],
    emissive: [f32; 3],
    emissive_strength: f32,
    flags: NeMaterialFlags,
    spec: &NeGameReadyMaterialSpec,
) -> (NeMaterialDescriptor, NeMaterialTextureBindings) {
    log::warn!(
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
        log::debug!(
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

/// Central game-ready material registration path.
///
/// Runtime materials are resolved only through `engine.assets.materials` from
/// `.nemat@entry` selectors. Hand-built texture slots are authoring diagnostics,
/// not runtime material sources. Missing/broken content receives an explicit
/// diagnostic material so the frame can still report what is wrong.
#[inline]
fn register_material(
    mats: &NeMaterialRegistry,
    name: &str,
    base_color: [f32; 4],
    emissive: [f32; 3],
    emissive_strength: f32,
    flags: NeMaterialFlags,
    spec: &NeGameReadyMaterialSpec,
) -> NeMaterialId {
    if let Some(asset_path) = spec.asset.as_deref() {
        if let Some(mut response) = load_material_descriptor_asset(asset_path) {
            response.descriptor.flags = response.descriptor.flags.union(flags);
            response.descriptor.sanitize_in_place();
            let material_name = if response.name.trim().is_empty() { name.to_owned() } else { response.name };
            log::debug!(
                "game-ready material: resolved .nemat material name='{}' source='{}' policy='ytyp->ydd->nemat->ytd'",
                material_name,
                response.source
            );
            return mats.upsert_named_with_textures(&material_name, response.descriptor, response.textures.sanitized());
        }
    } else {
        log::warn!(
            "game-ready material: missing material asset for name='{}' expected='<logical-path>.nemat@entry' policy='no runtime json/ad-hoc material source'",
            name
        );
    }

    let (desc, textures) = diagnostic_unresolved_material(name, base_color, emissive, emissive_strength, flags, spec);
    mats.upsert_named_with_textures(name, desc, textures)
}

