use newengine_render_api::{RenderGraphPassDomain, RenderGraphResourceUsage};

use crate::StandardRenderPhase;

use super::{
    FrameGraphBuilder, RG_GBUFFER_ALBEDO, RG_GBUFFER_DEPTH, RG_GBUFFER_MATERIAL, RG_GBUFFER_NORMAL,
};

impl FrameGraphBuilder {
    #[inline]
    pub fn lighting(mut self, deferred: bool) -> Self {
        if deferred {
            self = self.deferred_lighting();
        }
        self
    }

    #[inline]
    pub fn deferred_lighting(mut self) -> Self {
        let lit_output = self.viewport_color_resource();
        self.add_phase_pass(StandardRenderPhase::DeferredLighting, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d)
                .reads(RG_GBUFFER_ALBEDO, RenderGraphResourceUsage::SampledTexture)
                .reads(RG_GBUFFER_NORMAL, RenderGraphResourceUsage::SampledTexture)
                .reads(
                    RG_GBUFFER_MATERIAL,
                    RenderGraphResourceUsage::SampledTexture,
                )
                .reads(RG_GBUFFER_DEPTH, RenderGraphResourceUsage::SampledTexture)
                .writes(lit_output, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }
}
