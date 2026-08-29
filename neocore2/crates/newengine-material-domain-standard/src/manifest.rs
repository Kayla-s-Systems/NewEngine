use crate::manifest_root_policy;
use newengine_assets::AssetServiceClient;
use newengine_material_domain_api::{MaterialDomainError, MaterialDomainResult};
use newengine_plugin_host::default_host_api;
use newengine_render_api::ShaderSourceKind;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub(super) struct StandardLitShaderManifest {
    #[allow(dead_code)]
    schema: String,
    pub(super) shaders: StandardLitShaderSetManifest,
}

impl StandardLitShaderManifest {
    pub(super) fn load(logical_path: &str) -> MaterialDomainResult<Self> {
        let source = load_text_asset(logical_path)?;
        let manifest: Self = serde_json::from_str(&source).map_err(|e| {
            MaterialDomainError::other(format!(
                "Standard shader manifest parse failed path='{logical_path}' err='{e}'"
            ))
        })?;
        manifest.validate(logical_path)?;
        newengine_ulog_api::ulog::info!(
            "standard material domain: shader manifest loaded path='{}' schema='{}'",
            logical_path,
            manifest.schema
        );
        Ok(manifest)
    }

    fn validate(&self, logical_path: &str) -> MaterialDomainResult<()> {
        if self.schema.trim().is_empty() {
            return Err(MaterialDomainError::other(format!(
                "Standard shader manifest path='{logical_path}' missing schema"
            )));
        }
        self.shaders.validate(logical_path)
    }
}

#[derive(Clone, Deserialize)]
pub(super) struct StandardLitShaderSetManifest {
    pub(super) lit_vs: StandardShaderAssetRef,
    pub(super) lit_fs: StandardShaderAssetRef,
    pub(super) gbuffer_fs: StandardShaderAssetRef,
    pub(super) gbuffer_terrain_fs: StandardShaderAssetRef,
    pub(super) terrain_fs: StandardShaderAssetRef,
    pub(super) shadow_vs: StandardShaderAssetRef,
    pub(super) shadow_fs: StandardShaderAssetRef,
    pub(super) instanced_vs: StandardShaderAssetRef,
    pub(super) instanced_fs: StandardShaderAssetRef,
    pub(super) shadow_instanced_vs: StandardShaderAssetRef,
    pub(super) skinned_vs: StandardShaderAssetRef,
    pub(super) shadow_skinned_vs: StandardShaderAssetRef,
}

impl StandardLitShaderSetManifest {
    fn validate(&self, manifest_path: &str) -> MaterialDomainResult<()> {
        for (field, shader) in [
            ("lit_vs", &self.lit_vs),
            ("lit_fs", &self.lit_fs),
            ("gbuffer_fs", &self.gbuffer_fs),
            ("gbuffer_terrain_fs", &self.gbuffer_terrain_fs),
            ("terrain_fs", &self.terrain_fs),
            ("shadow_vs", &self.shadow_vs),
            ("shadow_fs", &self.shadow_fs),
            ("instanced_vs", &self.instanced_vs),
            ("instanced_fs", &self.instanced_fs),
            ("shadow_instanced_vs", &self.shadow_instanced_vs),
            ("skinned_vs", &self.skinned_vs),
            ("shadow_skinned_vs", &self.shadow_skinned_vs),
        ] {
            shader.validate(manifest_path, field)?;
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize)]
pub(super) struct StandardShaderAssetRef {
    pub(super) logical_path: String,
    #[serde(default = "default_source_kind")]
    pub(super) source_kind: String,
    #[serde(default = "default_entry")]
    pub(super) entry: String,
    #[serde(default = "default_variant")]
    pub(super) variant_id: String,
}

impl StandardShaderAssetRef {
    fn validate(&self, manifest_path: &str, field: &str) -> MaterialDomainResult<()> {
        if self.logical_path.trim().is_empty() {
            return Err(MaterialDomainError::other(format!(
                "Standard shader manifest path='{manifest_path}' field='{field}' has empty logical_path"
            )));
        }
        let _ = self.source_kind()?;
        manifest_root_policy::validate_manifest_shader_path(
            manifest_path,
            field,
            self.logical_path.as_str(),
        )?;
        Ok(())
    }

    pub(super) fn source_kind(&self) -> MaterialDomainResult<ShaderSourceKind> {
        match self.source_kind.trim().to_ascii_lowercase().as_str() {
            "glsl" => Ok(ShaderSourceKind::Glsl),
            "hlsl" => Ok(ShaderSourceKind::Hlsl),
            "wgsl" => Ok(ShaderSourceKind::Wgsl),
            "spirv" | "spv" => Ok(ShaderSourceKind::Spirv),
            other => Err(MaterialDomainError::other(format!(
                "unsupported shader source_kind='{other}' path='{}'",
                self.logical_path
            ))),
        }
    }
}

fn default_source_kind() -> String {
    "glsl".to_owned()
}

fn default_entry() -> String {
    "main".to_owned()
}

fn default_variant() -> String {
    "standard_default".to_owned()
}

fn load_text_asset(rel: &str) -> MaterialDomainResult<String> {
    let assets = AssetServiceClient::new(default_host_api());

    newengine_ulog_api::ulog::trace!(
        "asset text: requesting path='{rel}' through AssetManager.text_v1"
    );
    let payload = assets.text_v1(rel).map_err(|e| {
        MaterialDomainError::other(format!("asset.text_v1 failed path='{rel}' err='{e}'"))
    })?;

    let s = std::str::from_utf8(&payload)
        .map_err(|_| {
            MaterialDomainError::other(format!("asset.text_v1 returned non-utf8 path='{rel}'"))
        })?
        .to_string();

    newengine_ulog_api::ulog::trace!("asset text: loaded path='{rel}' bytes={}", payload.len());
    Ok(s)
}
