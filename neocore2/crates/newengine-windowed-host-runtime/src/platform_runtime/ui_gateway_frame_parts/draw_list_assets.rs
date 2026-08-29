use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use newengine_ui_api::{reserved, UiDrawList, UiPaintCommand, UiTexId, UiTexture};

static UI_ASSET_TEXTURE_RESIDENCY: OnceLock<Mutex<BTreeMap<UiTexId, String>>> = OnceLock::new();

fn residency() -> &'static Mutex<BTreeMap<UiTexId, String>> {
    UI_ASSET_TEXTURE_RESIDENCY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[inline]
pub(super) fn ui_asset_texture_id(texture_ref: &str) -> UiTexId {
    let mut hash = 0x811c_9dc5u32;
    for byte in texture_ref.trim().replace('\\', "/").as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    reserved::asset_ref_from_u32(hash)
}

fn is_resident(texture_id: UiTexId, texture_ref: &str) -> bool {
    residency()
        .lock()
        .ok()
        .and_then(|map| map.get(&texture_id).cloned())
        .is_some_and(|resident| resident == texture_ref)
}

fn mark_resident(texture_id: UiTexId, texture_ref: &str) {
    if let Ok(mut map) = residency().lock() {
        map.insert(texture_id, texture_ref.to_owned());
    }
}

#[cfg(test)]
pub(super) fn reset_ui_asset_texture_residency() {
    if let Ok(mut map) = residency().lock() {
        map.clear();
    }
}

/// Resolve authored `.ytd@entry` references emitted by UI providers into stable,
/// engine-managed UiTexIds and upload payloads. Providers remain layout/input/state
/// owners; YTD semantics stay behind engine.assets.textures.
pub(super) fn hydrate_ui_asset_textures(draw_list: &mut UiDrawList) {
    let mut required = BTreeMap::<UiTexId, String>::new();

    for command in &mut draw_list.paint.commands {
        match command {
            UiPaintCommand::Image(image) => {
                if let Some(texture_ref) = image
                    .texture_ref
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let texture_id = ui_asset_texture_id(texture_ref);
                    image.texture_id = Some(texture_id);
                    required
                        .entry(texture_id)
                        .or_insert_with(|| texture_ref.to_owned());
                }
            }
            UiPaintCommand::MaterialQuad(quad) => {
                if let Some(texture_ref) = quad
                    .texture_ref
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let texture_id = ui_asset_texture_id(texture_ref);
                    quad.texture_id = Some(texture_id);
                    required
                        .entry(texture_id)
                        .or_insert_with(|| texture_ref.to_owned());
                }
            }
            _ => {}
        }
    }

    if required.is_empty() {
        return;
    }

    let assets =
        newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    for (texture_id, texture_ref) in required {
        if draw_list.texture_delta.set.contains_key(&texture_id)
            || is_resident(texture_id, &texture_ref)
        {
            continue;
        }
        match assets.textures_entry_rgba8_ref_v1_typed(&texture_ref) {
            Ok(texture) => {
                let size = [texture.width, texture.height];
                let bytes = texture.rgba.len();
                draw_list.texture_delta.set.insert(
                    texture_id,
                    UiTexture {
                        size,
                        rgba8: texture.rgba,
                    },
                );
                mark_resident(texture_id, &texture_ref);
                newengine_ulog_api::ulog::debug!(
                    "ui asset texture resident ref='{}' tex_id={} size={}x{} bytes={} policy='engine.assets.textures.entry_rgba8_v1'",
                    texture_ref,
                    texture_id.0,
                    size[0],
                    size[1],
                    bytes
                );
            }
            Err(error) => {
                draw_list.paint.diagnostics.push(format!(
                    "ui texture resolve failed ref='{}' err='{}' policy='.ytd@entry only'",
                    texture_ref, error
                ));
                newengine_ulog_api::ulog::warn!(
                    "ui texture resolve failed ref='{}' err='{}' policy='.ytd@entry only'",
                    texture_ref,
                    error
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_ui_api::{UiImagePaintCommand, UiMaterialQuadPaintCommand, UiPaintCommand};

    #[test]
    fn ytd_entry_refs_get_stable_managed_ui_texture_ids() {
        let a = ui_asset_texture_id("textures/ui/menu/main_menu.ytd@background");
        let b = ui_asset_texture_id("textures/ui/menu/main_menu.ytd@background");
        let c = ui_asset_texture_id("textures/ui/menu/main_menu.ytd@selector");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(reserved::is_asset_ref(a));
        assert!(!reserved::is_external(a));
    }

    #[test]
    fn hydration_assigns_ids_before_asset_io() {
        reset_ui_asset_texture_residency();
        let mut list = UiDrawList::new();
        list.paint.push(UiPaintCommand::Image(UiImagePaintCommand {
            texture_ref: Some("missing/test.ytd@image".to_owned()),
            ..Default::default()
        }));
        list.paint
            .push(UiPaintCommand::MaterialQuad(UiMaterialQuadPaintCommand {
                texture_ref: Some("missing/test.ytd@button".to_owned()),
                ..Default::default()
            }));
        // Asset IO may fail in the isolated unit test, but deterministic ids are
        // assigned before resolution and diagnostics capture the failure.
        hydrate_ui_asset_textures(&mut list);
        match &list.paint.commands[0] {
            UiPaintCommand::Image(image) => assert!(image.texture_id.is_some()),
            _ => unreachable!(),
        }
        match &list.paint.commands[1] {
            UiPaintCommand::MaterialQuad(quad) => assert!(quad.texture_id.is_some()),
            _ => unreachable!(),
        }
    }
}
