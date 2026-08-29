use std::sync::Arc;

use newengine_material_domain_api::{MaterialGpuPipelineKey, MaterialGpuPipelineProvider};
use newengine_material_domain_standard::{
    StandardLitMaterialDomainProvider, STANDARD_LIT_PIPELINE_KEY,
};
use newengine_render_feature_api::{LightExtractionProvider, RenderDrawListProvider};

#[derive(Default)]
pub struct StandardRenderFeaturePack;

impl StandardRenderFeaturePack {
    #[inline]
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn runtime_contribution(&self) -> newengine_render_feature_api::RenderFeatureContribution {
        newengine_render_feature_api::RenderFeatureContribution::new(
            "newengine.render-feature.standard",
        )
        .with_draw_list_providers(self.draw_list_providers())
        .with_light_extraction_providers(self.light_extraction_providers())
        .with_material_pipeline(
            self.material_pipeline_provider(),
            self.primary_lit_material_domain(),
        )
    }

    #[inline]
    pub fn material_pipeline_provider(&self) -> Box<dyn MaterialGpuPipelineProvider> {
        Box::new(StandardLitMaterialDomainProvider::new())
    }

    #[inline]
    pub fn primary_lit_material_domain(&self) -> MaterialGpuPipelineKey {
        STANDARD_LIT_PIPELINE_KEY
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
