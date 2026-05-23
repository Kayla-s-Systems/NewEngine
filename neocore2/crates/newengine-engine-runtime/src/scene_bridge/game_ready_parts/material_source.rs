use newengine_materials::{
    method as material_method, MaterialDescriptor as NeMaterialDescriptor,
    MaterialDescriptorLoadResponse as NeMaterialDescriptorLoadResponse,
    MaterialFlags as NeMaterialFlags, MaterialId as NeMaterialId,
    MaterialLoadRequest as NeMaterialLoadRequest, MaterialRegistry as NeMaterialRegistry,
    MaterialTextureBindings as NeMaterialTextureBindings, ENGINE_MATERIALS_SERVICE_ID,
};

use self::content::GameReadyMaterialSpec as NeGameReadyMaterialSpec;

#[inline]
fn is_nemat_entry_ref(path: &str) -> bool {
    let value = path.trim().replace('\\', "/");
    let Some((file, entry)) = value.rsplit_once('@') else { return false; };
    !entry.trim().is_empty() && file.to_ascii_lowercase().ends_with(".nemat")
}

#[inline]
fn load_material_descriptor_asset(path: &str) -> Option<NeMaterialDescriptorLoadResponse> {
    if !is_nemat_entry_ref(path) {
        log::warn!(
            "game-ready material: rejected legacy/non-canonical material asset path='{}' expected='<logical-path>.nemat@entry' policy='ytyp->ydd->nemat->ytd' action='skip_asset'",
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
        ENGINE_MATERIALS_SERVICE_ID,
        material_method::LOAD_DESCRIPTOR_V1,
        &payload,
    ) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::warn!(
                "game-ready material: .nemat descriptor unavailable path='{}' gateway='engine.materials' method='{}' err='{}'",
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
        "game-ready material: unresolved material name='{}' asset={:?} policy='runtime requires .nemat@entry; json/ad-hoc material specs are legacy' action='register_diagnostic_material'",
        name,
        spec.asset
    );
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
/// Runtime materials are resolved only through `engine.materials` from
/// `.nemat@entry` selectors. Historical JSON material files and hand-built texture slots are
/// treated as legacy/importer data and are not consumed on the GameReady runtime
/// path. The only fallback is an explicit diagnostic material for missing/broken
/// content so the frame can still report what is wrong.
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

#[inline]
pub(super) fn game_ready_demo_enabled() -> bool {
    std::env::var("NEWENGINE_GAME_READY_DEMO")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "" | "0" | "false" | "off" | "no")
        })
        .unwrap_or(false)
}
