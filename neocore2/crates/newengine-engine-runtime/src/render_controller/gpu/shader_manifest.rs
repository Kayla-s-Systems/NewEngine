use newengine_assets::AssetServiceClient;
use newengine_core::render::ShaderSourceKind;
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_plugin_host::default_host_api;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub(super) struct RuntimeShaderProgramManifest {
    #[allow(dead_code)]
    pub schema: String,
    pub shaders: RuntimeShaderStages,
}

#[derive(Clone, Deserialize)]
pub(super) struct RuntimeShaderStages {
    pub vertex: RuntimeShaderStageRef,
    pub fragment: RuntimeShaderStageRef,
}

#[derive(Clone, Deserialize)]
pub(super) struct RuntimeShaderStageRef {
    pub logical_path: String,
    #[serde(default = "default_source_kind")]
    pub source_kind: String,
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default = "default_variant")]
    pub variant_id: String,
}

impl RuntimeShaderStageRef {
    pub(super) fn source_kind(&self) -> CoreResult<ShaderSourceKind> {
        match self.source_kind.trim().to_ascii_lowercase().as_str() {
            "glsl" => Ok(ShaderSourceKind::Glsl),
            "hlsl" => Ok(ShaderSourceKind::Hlsl),
            "wgsl" => Ok(ShaderSourceKind::Wgsl),
            "spirv" | "spv" => Ok(ShaderSourceKind::Spirv),
            other => Err(EngineError::other(format!(
                "runtime shader manifest: unsupported source_kind='{other}' path='{}'",
                self.logical_path
            ))),
        }
    }
}

pub(super) fn load_runtime_shader_program_manifest(logical_path: &str) -> CoreResult<RuntimeShaderProgramManifest> {
    let assets = AssetServiceClient::new(default_host_api());
    let payload = assets.text_v1(logical_path).map_err(|e| {
        EngineError::other(format!("runtime shader manifest load failed path='{logical_path}' err='{e}'"))
    })?;
    let text = std::str::from_utf8(&payload).map_err(|_| {
        EngineError::other(format!("runtime shader manifest is not UTF-8 path='{logical_path}'"))
    })?;
    let manifest: RuntimeShaderProgramManifest = serde_json::from_str(text).map_err(|e| {
        EngineError::other(format!("runtime shader manifest parse failed path='{logical_path}' err='{e}'"))
    })?;
    if manifest.shaders.vertex.logical_path.trim().is_empty() || manifest.shaders.fragment.logical_path.trim().is_empty() {
        return Err(EngineError::other(format!(
            "runtime shader manifest path='{logical_path}' must define vertex and fragment shader logical paths"
        )));
    }
    Ok(manifest)
}

fn default_source_kind() -> String { "glsl".to_owned() }
fn default_entry() -> String { "main".to_owned() }
fn default_variant() -> String { "runtime_default".to_owned() }
