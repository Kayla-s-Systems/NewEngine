use newengine_assets::AssetServiceClient;
use newengine_materials::{
    material_source_from_parts, parse_material_source_slice,
    MaterialDescriptor as NeMaterialDescriptor, MaterialFlags as NeMaterialFlags,
    MaterialId as NeMaterialId, MaterialRegistry as NeMaterialRegistry,
    MaterialSourceDocument as NeMaterialSourceDocument,
    MaterialTextureBindings as NeMaterialTextureBindings,
};
use newengine_plugin_host::default_host_api;

use self::content::GameReadyMaterialSpec as NeGameReadyMaterialSpec;

#[inline]
fn load_material_source_asset(path: &str) -> Option<NeMaterialSourceDocument> {
    let assets = AssetServiceClient::new(default_host_api());
    let payload = match assets.text_v1(path) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!(
                "game-ready material: source unavailable path='{}' method='asset.text_v1' err='{}'",
                path,
                e
            );
            return None;
        }
    };

    match parse_material_source_slice(&payload) {
        Ok(source) => Some(source),
        Err(e) => {
            log::warn!("game-ready material: source parse failed path='{}' err='{}'", path, e);
            None
        }
    }
}

#[inline]
fn material_texture_bindings(spec: &NeGameReadyMaterialSpec) -> NeMaterialTextureBindings {
    NeMaterialTextureBindings {
        base_color_texture: spec.base_color_texture.clone(),
        normal_texture: spec.normal_texture.clone(),
        metallic_texture: None,
        roughness_texture: spec.roughness_texture.clone(),
        occlusion_texture: None,
        emissive_texture: None,
        uv_scale: spec.uv_scale,
        uv_offset: spec.uv_offset,
    }
    .sanitized()
}

/// Central game-ready material registration path.
///
/// Scene profile JSON, material asset JSON and fallback defaults all pass through
/// this function before entering the `MaterialRegistry`. That gives the renderer
/// one canonical model: material texture slots may reference only `.ytd@entry`
/// dictionary selectors; source image containers are rejected by the material
/// crate sanitizer and never reach `AssetManager.import_v1`.
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
        if let Some(source) = load_material_source_asset(asset_path) {
            let source = source.with_fallback_name(name.to_owned());
            let mut desc = source.desc;
            desc.flags = desc.flags.union(flags);
            desc.sanitize_in_place();
            let material_name = source.name.clone().unwrap_or_else(|| name.to_owned());
            return mats.upsert_named_with_textures(&material_name, desc, source.textures.sanitized());
        }
    }

    let source = material_source_from_parts(
        name,
        NeMaterialDescriptor {
            base_color,
            emissive,
            emissive_strength,
            roughness: spec.roughness,
            normal_scale: spec.normal_scale,
            occlusion_strength: spec.occlusion_strength,
            flags,
            ..NeMaterialDescriptor::default()
        },
        material_texture_bindings(spec),
    );
    let material_name = source.name.clone().unwrap_or_else(|| name.to_owned());
    mats.upsert_named_with_textures(&material_name, source.desc, source.textures)
}
