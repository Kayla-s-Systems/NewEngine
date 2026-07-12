use std::sync::Arc;

use newengine_material_domain_api::{MaterialGpuPipelineKey, MaterialGpuPipelineProvider};
use newengine_material_domain_gameready::{
    GameReadyLitMaterialDomainProvider, GAME_READY_LIT_PIPELINE_KEY,
};
use newengine_render_feature_api::{LightExtractionProvider, RenderDrawListProvider};

#[derive(Default)]
pub struct GameReadyRenderFeaturePack;

impl GameReadyRenderFeaturePack {
    #[inline]
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn material_pipeline_provider(&self) -> Box<dyn MaterialGpuPipelineProvider> {
        Box::new(GameReadyLitMaterialDomainProvider::new())
    }

    #[inline]
    pub fn primary_lit_material_domain(&self) -> MaterialGpuPipelineKey {
        GAME_READY_LIT_PIPELINE_KEY
    }

    #[inline]
    pub fn draw_list_providers(&self) -> Vec<Arc<dyn RenderDrawListProvider>> {
        crate::draw::providers()
    }

    #[inline]
    pub fn light_extraction_providers(&self) -> Vec<Arc<dyn LightExtractionProvider>> {
        crate::lighting::providers()
    }
}
