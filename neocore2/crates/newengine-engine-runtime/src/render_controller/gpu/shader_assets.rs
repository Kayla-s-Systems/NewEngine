use newengine_assets::AssetServiceClient;
use newengine_core::render::ShaderStage;
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

    log::debug!("asset text: requesting path='{rel}' through AssetManager.text_v1");
    let payload = assets.text_v1(rel).map_err(|e| {
        EngineError::other(format!("asset.text_v1 failed path='{rel}' err='{e}'"))
    })?;

    let s = std::str::from_utf8(&payload)
        .map_err(|_| EngineError::other(format!("asset.text_v1 returned non-utf8 path='{rel}'")))?
        .to_string();

    log::debug!("asset text: loaded path='{rel}' bytes={}", payload.len());
    Ok(s)
}

pub(super) fn compile_glsl(stage: ShaderStage, name: &str, src: &str) -> CoreResult<Vec<u32>> {
    newengine_shader_compiler::compile_glsl_to_spirv(stage, name, "main", src)
        .map_err(|e| EngineError::other(format!("shader compile failed: {e}")))
}
