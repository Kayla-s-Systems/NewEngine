#![forbid(unsafe_op_in_unsafe_fn)]

//! Backward-compatible GameReady facade over the generic standard lit material domain.
//!
//! The implementation owner is `newengine-material-domain-standard`. This crate keeps
//! the historical Rust type and pipeline-key contract for existing consumers while
//! delegating all pipeline construction and caching to the standard provider.

use newengine_material_domain_api::{
    MaterialDomainResult, MaterialGpuPipeline, MaterialGpuPipelineKey, MaterialGpuPipelineProvider,
    MaterialPipelineBuildProfile, MaterialRenderDevice,
};
use newengine_material_domain_standard::StandardLitMaterialDomainProvider;

pub const GAME_READY_LIT_PIPELINE_KEY: MaterialGpuPipelineKey =
    MaterialGpuPipelineKey::new("newengine.material_domain.gameready.runtime_lit");

#[derive(Default)]
pub struct GameReadyLitMaterialDomainProvider {
    standard: StandardLitMaterialDomainProvider,
}

impl GameReadyLitMaterialDomainProvider {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }
}

impl MaterialGpuPipelineProvider for GameReadyLitMaterialDomainProvider {
    #[inline]
    fn key(&self) -> MaterialGpuPipelineKey {
        GAME_READY_LIT_PIPELINE_KEY
    }

    #[inline]
    fn require_pipeline(
        &mut self,
        profile: MaterialPipelineBuildProfile,
        render_device: &mut dyn MaterialRenderDevice,
    ) -> MaterialDomainResult<MaterialGpuPipeline> {
        self.standard.require_pipeline(profile, render_device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_key_is_preserved_while_implementation_is_standard() {
        let provider = GameReadyLitMaterialDomainProvider::new();
        assert_eq!(provider.key(), GAME_READY_LIT_PIPELINE_KEY);
        assert_ne!(
            provider.key(),
            newengine_material_domain_standard::STANDARD_LIT_PIPELINE_KEY
        );
    }
}
