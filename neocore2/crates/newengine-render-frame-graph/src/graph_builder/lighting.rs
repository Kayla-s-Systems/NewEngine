use newengine_render_api::{
    RenderGraphPassDomain, RenderGraphResourceDesc, RenderGraphResourceSemantic,
    RenderGraphResourceUsage,
};

use crate::StandardRenderPhase;

use super::{
    FrameGraphBuilder, RG_GBUFFER_ALBEDO, RG_GBUFFER_DEPTH, RG_GBUFFER_MATERIAL,
    RG_GBUFFER_NORMAL, RG_LIT_COLOR,
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
        self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
            RG_LIT_COLOR,
            "lit_color",
            RenderGraphResourceUsage::ColorAttachment,
            self.target.viewport_extent,
            self.target.scene_color_format,
        )
        .with_semantic(RenderGraphResourceSemantic::LitColor));
        self.add_phase_pass(StandardRenderPhase::DeferredLighting, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d).reads(RG_GBUFFER_ALBEDO, RenderGraphResourceUsage::SampledTexture)
                .reads(RG_GBUFFER_NORMAL, RenderGraphResourceUsage::SampledTexture)
                .reads(RG_GBUFFER_MATERIAL, RenderGraphResourceUsage::SampledTexture)
                .reads(RG_GBUFFER_DEPTH, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_LIT_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

}
