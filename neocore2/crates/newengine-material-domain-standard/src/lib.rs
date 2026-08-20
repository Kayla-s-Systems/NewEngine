#![forbid(unsafe_op_in_unsafe_fn)]

//! Standard/FPS runtime-lit material-domain provider.
//!
//! Provider orchestration is intentionally separated from shader-manifest parsing and
//! pipeline construction. The renderer remains the owner of shader compilation.

use std::collections::HashMap;

mod manifest;
mod manifest_root_policy;
mod pipeline;

use manifest::StandardLitShaderManifest;
use newengine_material_domain_api::{
    LitPipeline, MaterialDomainResult, MaterialGpuPipeline, MaterialGpuPipelineKey,
    MaterialGpuPipelineProvider, MaterialPipelineBuildProfile, MaterialRenderDevice,
};
use pipeline::PendingLitPipelineBuild;

pub const STANDARD_LIT_PIPELINE_KEY: MaterialGpuPipelineKey =
    MaterialGpuPipelineKey::new("newengine.material_domain.standard.runtime_lit");

const DEFAULT_SHADER_MANIFEST_PATH: &str = "shaders/pipelines/gameready_lit.pipeline.json";

#[derive(Default)]
pub struct StandardLitMaterialDomainProvider {
    manifest: Option<StandardLitShaderManifest>,
    pipelines: HashMap<String, LitPipeline>,
    pending_pipelines: HashMap<String, PendingLitPipelineBuild>,
}

impl StandardLitMaterialDomainProvider {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    fn require_manifest(&mut self) -> MaterialDomainResult<StandardLitShaderManifest> {
        if let Some(manifest) = self.manifest.clone() {
            return Ok(manifest);
        }
        let manifest = StandardLitShaderManifest::load(DEFAULT_SHADER_MANIFEST_PATH)?;
        self.manifest = Some(manifest.clone());
        Ok(manifest)
    }
}

impl MaterialGpuPipelineProvider for StandardLitMaterialDomainProvider {
    #[inline]
    fn key(&self) -> MaterialGpuPipelineKey {
        STANDARD_LIT_PIPELINE_KEY
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

        if !self.pending_pipelines.contains_key(&cache_key) {
            let manifest = self.require_manifest()?;
            self.pending_pipelines.insert(
                cache_key.clone(),
                PendingLitPipelineBuild::new(profile, manifest),
            );
            newengine_ulog_api::ulog::info!(
                "standard material domain: staged pipeline warmup started cache_key='{}' deferred_pipelines={} policy='bounded loading-frame work'",
                cache_key,
                profile.deferred_pipelines,
            );
        }

        let build = self
            .pending_pipelines
            .get_mut(&cache_key)
            .expect("pending pipeline inserted");
        match build.advance(r)? {
            Some(pipeline) => {
                self.pending_pipelines.remove(&cache_key);
                self.pipelines.insert(cache_key, pipeline);
                Ok(MaterialGpuPipeline::Lit(pipeline))
            }
            None => Err(newengine_material_domain_api::MaterialDomainError::other(format!(
                "pipeline warmup pending cache_key='{cache_key}' policy='bounded loading-frame work'"
            ))),
        }
    }
}

#[inline]
fn profile_pipeline_cache_key(profile: MaterialPipelineBuildProfile) -> String {
    format!(
        "scene={:?}|shadow={:?}",
        profile.scene_hdr_color_format, profile.shadow_map_color_format
    )
}
