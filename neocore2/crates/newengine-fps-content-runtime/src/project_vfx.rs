use std::collections::BTreeSet;

use newengine_engine_runtime::gameplay::{GameplayWorld, ItemCatalog};
use newengine_vfx_api::VfxGpuTextureRegistry;
use newengine_vfx_runtime::VfxEffectLibrary;

pub(crate) fn install_empty_project_vfx_resources(world: &mut GameplayWorld) {
    // Explicit degraded state: unresolved project effects stay unresolved. This is not a
    // generated/default gameplay effect fallback; it only prevents stale VFX resources from a
    // previous content generation from surviving a failed project dictionary load.
    world.insert_resource(VfxEffectLibrary::default());
    world.insert_resource(VfxGpuTextureRegistry::default());
}

pub(crate) fn install_project_vfx_dictionaries(
    world: &mut GameplayWorld,
    catalog: &ItemCatalog,
) -> Result<(), String> {
    let mut effect_refs = BTreeSet::<String>::new();
    for definition in catalog.definitions() {
        effect_refs.extend(definition.weapon_vfx.effect_refs().map(str::to_owned));
    }

    let mut library = VfxEffectLibrary::default();
    let mut textures = VfxGpuTextureRegistry::default();
    if effect_refs.is_empty() {
        world.insert_resource(library);
        world.insert_resource(textures);
        return Ok(());
    }

    let assets =
        newengine_assets_api::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let mut dictionaries = BTreeSet::<String>::new();
    for effect_ref in &effect_refs {
        let (reference, descriptor) =
            assets.require_semantic_asset_reference_v1(effect_ref, "engine.render.vfx", true)?;
        if descriptor.asset_kind != "effect_dictionary" {
            return Err(format!(
                "project weapon VFX reference '{}' resolves to asset kind '{}', expected effect_dictionary",
                effect_ref, descriptor.asset_kind
            ));
        }
        dictionaries.insert(reference.logical_path);
    }

    for path in dictionaries {
        let descriptor = assets.resolve_file_type_v1(&path)?;
        let content_kind = descriptor.content_kind.ok_or_else(|| {
            format!(
                "VFX format module '{}' does not declare NEF8 content_kind",
                descriptor.module_id
            )
        })?;
        let schema_version = descriptor.content_schema_version.ok_or_else(|| {
            format!(
                "VFX format module '{}' does not declare content_schema_version",
                descriptor.module_id
            )
        })?;
        let bytes = assets.raw_bytes_v1(&path).map_err(|error| {
            format!("project VFX dictionary load failed path='{path}' err='{error}'")
        })?;
        let dictionary =
            newengine_asset_format_nef8::decode_fxd_nef8(&bytes, content_kind, schema_version)
                .map_err(|error| {
                    format!("project VFX dictionary decode failed path='{path}' err='{error}'")
                })?;
        library.register_fxd_dictionary(&dictionary, &path, &mut textures)?;
    }

    for effect_ref in &effect_refs {
        if library.get(effect_ref).is_none() {
            return Err(format!(
                "project weapon references missing FXD effect '{effect_ref}'"
            ));
        }
    }

    world.insert_resource(library);
    world.insert_resource(textures);
    Ok(())
}
