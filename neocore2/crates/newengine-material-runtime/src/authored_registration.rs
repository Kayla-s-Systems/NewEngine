use newengine_material_domain_api::AuthoredMaterialSpec;
use newengine_materials::{
    api::MaterialRegistryApi, MaterialDescriptor, MaterialDescriptorLoadResponse, MaterialFlags,
    MaterialId, MaterialRegistry, MaterialTextureBindings,
};

use crate::MaterialAssetGatewayAdapter;

#[inline]
pub fn is_nemat_entry_ref(path: &str) -> bool {
    let value = path.trim().replace('\\', "/");
    newengine_assets::require_asset_reference_extension(&value, &["nemat"], true).is_ok()
}

#[inline]
pub fn load_authored_material_descriptor_asset(
    path: &str,
) -> Option<MaterialDescriptorLoadResponse> {
    if !is_nemat_entry_ref(path) {
        newengine_ulog_api::ulog::warn!(
            "authored material: rejected non-canonical material asset path='{}' expected='<logical-path>.nemat@entry' action='skip_asset'",
            path
        );
        return None;
    }
    let client =
        newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let adapter = MaterialAssetGatewayAdapter::with_client(client);
    let request = newengine_materials::MaterialLoadRequest {
        logical_path: path.trim().replace('\\', "/"),
        selector: None,
    };
    match adapter.load_descriptor(&request) {
        Ok(response) => Some(response),
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "authored material: descriptor unavailable path='{}' err='{}'",
                path,
                error
            );
            None
        }
    }
}

#[inline]
pub fn diagnostic_unresolved_material(
    name: &str,
    base_color: [f32; 4],
    emissive: [f32; 3],
    emissive_strength: f32,
    flags: MaterialFlags,
    spec: &AuthoredMaterialSpec,
) -> (MaterialDescriptor, MaterialTextureBindings) {
    newengine_ulog_api::ulog::warn!(
        "authored material: unresolved name='{}' asset={:?} action='register_diagnostic_material'",
        name,
        spec.asset
    );
    let mut desc = MaterialDescriptor {
        base_color,
        emissive,
        emissive_strength,
        roughness: spec.roughness,
        normal_scale: spec.normal_scale,
        occlusion_strength: spec.occlusion_strength,
        flags,
        ..MaterialDescriptor::default()
    };
    desc.sanitize_in_place();
    (desc, MaterialTextureBindings::default())
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
    materials: &MaterialRegistry,
    fallback_name: &str,
    flags: MaterialFlags,
    mut response: MaterialDescriptorLoadResponse,
) -> MaterialId {
    response.descriptor.flags = response.descriptor.flags.union(flags);
    response.descriptor.sanitize_in_place();
    let registry_name = material_registry_identity(&response.source, fallback_name);
    materials.upsert_named_with_textures(
        &registry_name,
        response.descriptor,
        response.textures.sanitized(),
    )
}

#[inline]
pub fn register_required_material_ref(
    materials: &MaterialRegistry,
    name: &str,
    flags: MaterialFlags,
    asset_path: &str,
) -> Result<MaterialId, String> {
    let asset_path = asset_path.trim();
    if asset_path.is_empty() {
        return Err(format!(
            "required material '{name}' has an empty asset reference"
        ));
    }
    let canonical = newengine_assets::require_asset_reference_extension(
        asset_path,
        &["nemat"],
        true,
    )
    .map_err(|error| {
        format!("required material '{name}' has invalid asset reference '{asset_path}': {error}")
    })?
    .canonical;
    if let Some(id) = materials.id_by_name(&canonical) {
        if let Some(mut resolved) = materials.resolve(id) {
            let merged_flags = resolved.desc.flags.union(flags);
            if merged_flags != resolved.desc.flags {
                resolved.desc.flags = merged_flags;
                materials.set_desc(id, resolved.desc).map_err(|error| {
                    format!(
                        "update cached material flags name='{name}' asset='{canonical}': {error}"
                    )
                })?;
            }
        }
        return Ok(id);
    }
    let response = load_authored_material_descriptor_asset(&canonical).ok_or_else(|| {
        format!(
            "required material descriptor unavailable name='{}' asset='{}'",
            name, canonical
        )
    })?;
    Ok(upsert_loaded_material(materials, name, flags, response))
}

#[inline]
pub fn register_required_material(
    materials: &MaterialRegistry,
    name: &str,
    flags: MaterialFlags,
    spec: &AuthoredMaterialSpec,
) -> Result<MaterialId, String> {
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
    register_required_material_ref(materials, name, flags, asset_path)
}

#[inline]
pub fn register_authored_material(
    materials: &MaterialRegistry,
    name: &str,
    base_color: [f32; 4],
    emissive: [f32; 3],
    emissive_strength: f32,
    flags: MaterialFlags,
    spec: &AuthoredMaterialSpec,
) -> MaterialId {
    if let Some(asset_path) = spec.asset.as_deref() {
        if let Some(response) = load_authored_material_descriptor_asset(asset_path) {
            return upsert_loaded_material(materials, name, flags, response);
        }
    }
    let (desc, textures) =
        diagnostic_unresolved_material(name, base_color, emissive, emissive_strength, flags, spec);
    materials.upsert_named_with_textures(name, desc, textures)
}

#[cfg(test)]
mod tests {
    use super::{material_registry_identity, register_required_material_ref};
    use newengine_materials::{
        api::MaterialRegistryApi, MaterialDescriptor, MaterialFlags, MaterialRegistry,
    };

    #[test]
    fn registry_identity_is_scoped_by_canonical_nemat_source() {
        assert_eq!(
            material_registry_identity("shared/materials/weapon_rifle.nemat@m00", "m00"),
            "shared/materials/weapon_rifle.nemat@m00"
        );
        assert_ne!(
            material_registry_identity("shared/materials/weapon_rifle.nemat@m00", "m00"),
            material_registry_identity(
                "shared/materials/characters/sample_character.nemat@m00",
                "m00"
            )
        );
    }

    #[test]
    fn required_material_ref_hits_registry_before_asset_gateway() {
        let materials = MaterialRegistry::new();
        let asset = "materials/world/test.nemat@m00";
        let id = materials.register_named(asset, MaterialDescriptor::default());

        let resolved = register_required_material_ref(
            &materials,
            "World/Test",
            MaterialFlags::CAST_SHADOWS,
            asset,
        )
        .expect("cached authored material should not require AssetManager");

        assert_eq!(resolved, id);
        assert!(materials
            .resolve(id)
            .expect("cached material")
            .desc
            .flags
            .contains(MaterialFlags::CAST_SHADOWS));
    }
}
