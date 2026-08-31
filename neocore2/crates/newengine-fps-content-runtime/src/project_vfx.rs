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

    let mut dictionaries = BTreeSet::<String>::new();
    for effect_ref in &effect_refs {
        let (path, selector) = effect_ref.rsplit_once('@').ok_or_else(|| {
            format!("project weapon VFX reference must use file.fxd@effect syntax: '{effect_ref}'")
        })?;
        if path.trim().is_empty()
            || selector.trim().is_empty()
            || !path.trim().to_ascii_lowercase().ends_with(".fxd")
        {
            return Err(format!(
                "invalid project weapon VFX reference '{effect_ref}'; expected file.fxd@effect"
            ));
        }
        dictionaries.insert(path.trim().replace('\\', "/"));
    }

    let assets =
        newengine_assets_api::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    for path in dictionaries {
        let bytes = assets
            .raw_bytes_v1(&path)
            .map_err(|error| format!("project FXD load failed path='{path}' err='{error}'"))?;
        let dictionary = newengine_asset_format_nef8::decode_fxd_nef8(&bytes)
            .map_err(|error| format!("project FXD decode failed path='{path}' err='{error}'"))?;
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
