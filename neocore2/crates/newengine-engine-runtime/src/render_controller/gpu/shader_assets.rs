use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_core::render::{Extent2D, ShaderStage};
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_plugin_host::default_host_api;

/// Load a UTF-8 runtime asset through AssetManager/VFS only.
///
/// There are deliberately no embedded shader/text fallbacks in this module:
/// loose files and `.pak` layers must be the single source of truth for runtime
/// assets. If this fails, the caller receives a hard diagnostic with the logical
/// path that AssetManager could not resolve.
pub(super) fn load_text_asset(rel: &str) -> CoreResult<String> {
    let assets = AssetServiceClient::new(default_host_api());

    log::debug!("asset text: requesting path='{rel}'");
    let id = assets.load(rel).map_err(|e| {
        EngineError::other(format!("asset.load failed path='{rel}' err='{e}'"))
    })?;

    wait_ready(&assets, &id, std::time::Duration::from_secs(2)).map_err(|e| {
        EngineError::other(format!(
            "asset not ready path='{rel}' id='{id}' err='{e:?}'"
        ))
    })?;

    let (_meta, payload) = assets.blob_wire_v1(&id).map_err(|e| {
        EngineError::other(format!(
            "asset.blob_wire_v1 failed path='{rel}' id='{id}' err='{e}'"
        ))
    })?;

    let s = std::str::from_utf8(&payload)
        .map_err(|_| EngineError::other(format!("asset is not utf8 path='{rel}'")))?
        .to_string();

    log::debug!(
        "asset text: loaded path='{rel}' id='{id}' bytes={}",
        payload.len()
    );
    Ok(s)
}

#[cfg(feature = "texture-decode")]
#[allow(dead_code)]
pub fn load_rgba_texture_asset(rel: &str) -> CoreResult<(Extent2D, Vec<u8>)> {
    let assets = AssetServiceClient::new(default_host_api());
    log::debug!("asset texture: requesting path='{rel}'");
    let id = assets
        .load(rel)
        .map_err(|e| EngineError::other(format!("asset.load failed path='{rel}' err='{e}'")))?;
    wait_ready(&assets, &id, std::time::Duration::from_secs(3)).map_err(|e| {
        EngineError::other(format!(
            "asset not ready path='{rel}' id='{id}' err='{e:?}'"
        ))
    })?;
    let (_meta, payload) = assets.blob_wire_v1(&id).map_err(|e| {
        EngineError::other(format!(
            "asset.blob_wire_v1 failed path='{rel}' id='{id}' err='{e}'"
        ))
    })?;
    let dyn_img = image::load_from_memory(&payload)
        .map_err(|e| EngineError::other(format!("image decode failed path='{rel}' err='{e}'")))?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    log::debug!(
        "asset texture: decoded path='{rel}' id='{id}' size={}x{} bytes={}",
        w,
        h,
        payload.len()
    );
    Ok((Extent2D::new(w, h), rgba.into_raw()))
}

#[cfg(not(feature = "texture-decode"))]
#[allow(dead_code)]
pub(super) fn load_rgba_texture_asset(rel: &str) -> CoreResult<(Extent2D, Vec<u8>)> {
    Err(EngineError::other(format!(
        "texture decode requested in runtime-core for '{rel}', but image decoding must go through AssetManager/imageImporter"
    )))
}

pub(super) fn compile_glsl(stage: ShaderStage, name: &str, src: &str) -> CoreResult<Vec<u32>> {
    newengine_shader_compiler::compile_glsl_to_spirv(stage, name, "main", src)
        .map_err(|e| EngineError::other(format!("shader compile failed: {e}")))
}
