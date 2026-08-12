#![forbid(unsafe_op_in_unsafe_fn)]

//! GameReady/FPS runtime-lit material-domain provider.
//!
//! Provider orchestration is intentionally separated from shader-manifest parsing and
//! pipeline construction. The renderer remains the owner of shader compilation.

use std::collections::HashMap;

mod manifest;
mod manifest_root_policy;
mod pipeline;

use manifest::GameReadyLitShaderManifest;
use newengine_material_domain_api::{
    LitPipeline, MaterialDomainResult, MaterialGpuPipeline, MaterialGpuPipelineKey,
    MaterialGpuPipelineProvider, MaterialPipelineBuildProfile, MaterialRenderDevice,
};

pub const GAME_READY_LIT_PIPELINE_KEY: MaterialGpuPipelineKey =
    MaterialGpuPipelineKey::new("newengine.material_domain.gameready.runtime_lit");

const DEFAULT_SHADER_MANIFEST_PATH: &str = "shaders/pipelines/gameready_lit.pipeline.json";

#[derive(Default)]
pub struct GameReadyLitMaterialDomainProvider {
    manifest: Option<GameReadyLitShaderManifest>,
    pipelines: HashMap<String, LitPipeline>,
}

impl GameReadyLitMaterialDomainProvider {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    fn require_manifest(&mut self) -> MaterialDomainResult<GameReadyLitShaderManifest> {
        if let Some(manifest) = self.manifest.clone() {
            return Ok(manifest);
        }
        let manifest = GameReadyLitShaderManifest::load(DEFAULT_SHADER_MANIFEST_PATH)?;
        self.manifest = Some(manifest.clone());
        Ok(manifest)
    }
}

impl MaterialGpuPipelineProvider for GameReadyLitMaterialDomainProvider {
    #[inline]
    fn key(&self) -> MaterialGpuPipelineKey {
        GAME_READY_LIT_PIPELINE_KEY
    }

    fn require_pipeline(
        &mut self,
        profile: MaterialPipelineBuildProfile,
        r: &mut dyn MaterialRenderDevice,
    ) -> MaterialDomainResult<MaterialGpuPipeline> {
        let cache_key = profile_pipeline_cache_key(profile);
        if let Some(pipeline) = self.pipelines.get(&cache_key).copied() {
            return Ok(MaterialGpuPipeline::Lit(pipeline));
        }

        let pipeline = self.build_pipeline(profile, r)?;
        self.pipelines.insert(cache_key, pipeline);
        Ok(MaterialGpuPipeline::Lit(pipeline))
    }
}

#[inline]
fn profile_pipeline_cache_key(profile: MaterialPipelineBuildProfile) -> String {
    format!(
        "scene={:?}|shadow={:?}",
        profile.scene_hdr_color_format, profile.shadow_map_color_format
    )
}
