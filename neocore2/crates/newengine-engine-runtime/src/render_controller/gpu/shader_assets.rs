use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_core::render::Extent2D;
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_plugin_host::default_host_api;

pub(super) fn load_text_asset(rel: &str) -> CoreResult<String> {
    // Hard rule: assets are loaded only through AssetManager/VFS so `.pak` layering works.
    // Renderer-side shader IO stays behind AssetManager/VFS so loose files and `.pak` remain interchangeable.
    let assets = AssetServiceClient::new(default_host_api());

    let id = match assets.load(rel) {
        Ok(id) => id,
        Err(e) => {
            if let Some(fallback) = builtin_text_asset(rel) {
                log::warn!("asset.load failed, using builtin fallback path='{rel}' err='{e}'");
                return Ok(fallback.to_string());
            }
            return Err(EngineError::other(format!(
                "asset.load failed path='{rel}' err='{e}'"
            )));
        }
    };

    if let Err(e) = wait_ready(&assets, &id, std::time::Duration::from_secs(2)) {
        if let Some(fallback) = builtin_text_asset(rel) {
            log::warn!("asset not ready, using builtin fallback path='{rel}' err='{e:?}'");
            return Ok(fallback.to_string());
        }
        return Err(EngineError::other(format!(
            "asset not ready path='{rel}' id='{id}' err='{e:?}'"
        )));
    }

    let (_meta, payload) = match assets.blob_wire_v1(&id) {
        Ok(v) => v,
        Err(e) => {
            if let Some(fallback) = builtin_text_asset(rel) {
                log::warn!("asset.blob_wire_v1 failed, using builtin fallback path='{rel}' err='{e}'");
                return Ok(fallback.to_string());
            }
            return Err(EngineError::other(format!(
                "asset.blob_wire_v1 failed path='{rel}' err='{e}'"
            )));
        }
    };

    let s = std::str::from_utf8(&payload)
        .map_err(|_| EngineError::other(format!("asset is not utf8 path='{rel}'")))?
        .to_string();

    Ok(s)
}


#[cfg(feature = "texture-decode")]
pub fn load_rgba_texture_asset(rel: &str) -> CoreResult<(Extent2D, Vec<u8>)> {
    let assets = AssetServiceClient::new(default_host_api());
    let id = assets.load(rel).map_err(|e| EngineError::other(format!("asset.load failed path='{rel}' err='{e}'")))?;
    wait_ready(&assets, &id, std::time::Duration::from_secs(3))
        .map_err(|e| EngineError::other(format!("asset not ready path='{rel}' err='{e:?}'")))?;
    let (_meta, payload) = assets
        .blob_wire_v1(&id)
        .map_err(|e| EngineError::other(format!("asset.blob_wire_v1 failed path='{rel}' err='{e}'")))?;
    let dyn_img = image::load_from_memory(&payload)
        .map_err(|e| EngineError::other(format!("image decode failed path='{rel}' err='{e}'")))?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((Extent2D::new(w, h), rgba.into_raw()))
}

#[cfg(not(feature = "texture-decode"))]
pub(super) fn load_rgba_texture_asset(rel: &str) -> CoreResult<(Extent2D, Vec<u8>)> {
    Err(EngineError::other(format!(
        "texture decode requested in runtime-core for '{rel}', but image decoding must go through AssetManager/imageImporter"
    )))
}

mod shader_builtins;
use self::shader_builtins::builtin_text_asset;
pub(super) use self::shader_builtins::{compile_glsl, BUILTIN_DEBUG_LINES_FRAG, BUILTIN_DEBUG_LINES_VERT};
