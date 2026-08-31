use super::super::*;
use newengine_materials::api::MaterialRegistryApi;

/// Resolve an optional prefab-level authored material. This is a project/content declaration,
/// never a GameReady material fallback.
pub(super) fn register_authored_prefab_material(
    mats: &MaterialRegistry,
    prefab: &GameReadyPrefabSpec,
) -> Result<Option<MaterialId>, String> {
    let raw_asset = prefab.material.trim();
    if raw_asset.is_empty() {
        return Ok(None);
    }
    let asset =
        newengine_assets_api::require_asset_reference_extension(raw_asset, &["nemat"], true)
            .map_err(|error| {
                format!(
                    "static world prefab id='{}' authored material='{}' rejected: {}",
                    prefab.id, raw_asset, error
                )
            })?
            .canonical;
    let spec = GameReadyMaterialSpec {
        asset: Some(asset),
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness: 1.0,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
    };
    let flags = MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS);
    Ok(Some(register_required_material(
        mats,
        &format!("World/Static/{}/AuthoredMaterial", prefab.id),
        flags,
        &spec,
    )?))
}

/// Resolve a mesh material from authored YDD/NEMAT metadata only.
///
/// The engine deliberately does not infer material classes from mesh-slot names and does not
/// synthesize road/terrain/props materials. A part without a YDD material reference may use the
/// explicit prefab-level material declaration; if neither exists, project content is incomplete.
pub(super) fn resolve_prefab_part_material(
    mats: &MaterialRegistry,
    authored_material: Option<MaterialId>,
    material_slot: &str,
    material_ref: Option<&str>,
) -> Result<(MaterialId, newengine_model_domain_api::MeshRenderOptions), String> {
    let static_flags = MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS);
    let material_id = if let Some(reference) = material_ref
        .map(str::trim)
        .filter(|reference| !reference.is_empty())
    {
        if !is_nemat_entry_ref(reference) {
            return Err(format!(
                "static world part slot='{}' material_ref='{}' must be a selector-qualified NEMAT entry",
                material_slot, reference
            ));
        }
        register_required_material_ref(
            mats,
            &format!("World/Static/{material_slot}"),
            static_flags,
            reference,
        )?
    } else {
        authored_material.ok_or_else(|| {
            format!(
                "static world part slot='{}' has no authored material_ref and prefab declares no material",
                material_slot
            )
        })?
    };

    let material = mats.resolve(material_id).ok_or_else(|| {
        format!(
            "static world material registry lost resolved material for slot='{}' id={:?}",
            material_slot, material_id
        )
    })?;
    let render_options = if material.desc.flags.contains(MaterialFlags::ALPHA_TEST) {
        newengine_model_domain_api::MeshRenderOptions::world_masked()
    } else {
        newengine_model_domain_api::MeshRenderOptions::world_opaque()
    };
    Ok((material_id, render_options))
}
