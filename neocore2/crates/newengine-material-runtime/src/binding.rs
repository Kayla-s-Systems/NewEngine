use newengine_materials::{
    validate_material_texture_reference, MaterialDescriptor, MaterialFlags, MaterialTextureBindings,
};
use newengine_model_domain_api::ModelMaterialBinding;
use newengine_model_import_obj::ModelMaterialSource;

pub fn material_binding(
    material_slot: &str,
    parsed: Option<&ModelMaterialSource>,
    _texture_dictionary: Option<&str>,
) -> ModelMaterialBinding {
    let mut descriptor = parsed
        .map(|mat| MaterialDescriptor {
            base_color: [mat.kd[0], mat.kd[1], mat.kd[2], mat.alpha],
            roughness: (1.0 - (mat.ns / 512.0).clamp(0.0, 0.9)).clamp(0.28, 0.92),
            flags: MaterialFlags::DOUBLE_SIDED
                .union(MaterialFlags::CAST_SHADOWS)
                .union(MaterialFlags::RECEIVE_SHADOWS)
                .union(if mat.alpha < 0.99 {
                    MaterialFlags::ALPHA_BLEND
                } else {
                    MaterialFlags::NONE
                }),
            ..MaterialDescriptor::default()
        })
        .unwrap_or_default();
    descriptor.sanitize_in_place();

    let mut textures = MaterialTextureBindings::default();
    if let Some(texture) = parsed
        .and_then(|mat| mat.base_color_texture.as_deref())
        .and_then(strict_runtime_texture_ref)
    {
        textures.base_color_texture = Some(texture);
    }
    if let Some(texture) = parsed
        .and_then(|mat| mat.normal_texture.as_deref())
        .and_then(strict_runtime_texture_ref)
    {
        textures.normal_texture = Some(texture);
    }

    let fallback_color = descriptor.base_color;
    ModelMaterialBinding {
        slot: material_slot.to_owned(),
        descriptor,
        textures: textures.sanitized(),
        fallback_color,
        material_ref: None,
        resolution_policy: "runtime_strict_ydd_nemat_ytd_chain".to_owned(),
    }
}

/// Runtime material paths accept only already-authored `.ytd@entry` selectors.
///
/// Deriving a texture entry from `*.dds`, `*.png`, `*.jpg` or an OBJ/MTL source
/// filename is importer/migration tooling behavior. It is intentionally absent
/// from this runtime helper so model/material hot paths cannot silently stitch
/// source images into authored material graphs.
pub fn strict_runtime_texture_ref(path: &str) -> Option<String> {
    match validate_material_texture_reference(path) {
        Ok(reference) => Some(reference.canonical),
        Err(error) => {
            newengine_ulog_api::ulog::debug!(
                "materials.runtime: rejected non-runtime texture ref path='{}' reason='{}' policy='.ytd@entry only'",
                path,
                error
            );
            None
        }
    }
}
