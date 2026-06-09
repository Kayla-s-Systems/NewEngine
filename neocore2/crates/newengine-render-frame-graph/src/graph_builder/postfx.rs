use newengine_render_api::{
    RenderGraphPassDesc, RenderGraphPassDomain, RenderGraphPassKind, RenderGraphResourceId,
    RenderGraphResourceUsage,
};

use crate::StandardRenderPhase;

use super::{FrameGraphBuilder, RG_LIT_COLOR, RG_SCENE_HDR_COLOR, RG_SURFACE_COLOR};

impl FrameGraphBuilder {
    pub fn postfx(mut self, enabled: bool) -> Self {
        if !enabled {
            return self;
        }

        let Some(input) = self.sampleable_scene_input_resource() else {
            return self;
        };
        self.add_phase_pass(StandardRenderPhase::PostFx, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(input, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn bloom_extract(mut self) -> Self {
        let Some(input) = self.sampleable_scene_input_resource() else {
            return self;
        };
        self.add_phase_pass(StandardRenderPhase::BloomExtract, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(input, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn bloom_blur(mut self) -> Self {
        self.add_phase_pass(StandardRenderPhase::BloomBlur, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(RG_SURFACE_COLOR, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn taa_resolve(mut self) -> Self {
        self.add_phase_pass(StandardRenderPhase::TaaResolve, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(RG_SURFACE_COLOR, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn msaa_resolve(mut self) -> Self {
        self.add_phase_pass(StandardRenderPhase::MsaaResolve, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(RG_SURFACE_COLOR, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    pub(super) fn finalize_surface_output(&mut self) {
        if !self.target.hdr_scene_enabled || self.has_surface_color_writer() {
            return;
        }

        let Some(input) = self.sampleable_scene_input_resource() else {
            return;
        };

        let id = newengine_render_api::RenderGraphPassId(self.next_custom_pass);
        self.next_custom_pass = self.next_custom_pass.saturating_add(1);
        let pass = RenderGraphPassDesc::new(id, "hdr_scene_resolve_to_surface", RenderGraphPassKind::Copy)
            .with_domain(RenderGraphPassDomain::PostProcess)
            .reads(input, RenderGraphResourceUsage::SampledTexture)
            .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment);
        self.graph.passes.push(pass);
    }

    #[inline]
    fn has_surface_color_writer(&self) -> bool {
        self.graph
            .passes
            .iter()
            .any(|pass| pass.writes.iter().any(|write| write.resource == RG_SURFACE_COLOR))
    }

    #[inline]
    pub(super) fn sampleable_scene_input_resource(&self) -> Option<RenderGraphResourceId> {
        if self.has_resource(RG_LIT_COLOR) {
            return Some(RG_LIT_COLOR);
        }
        if self.has_resource(RG_SCENE_HDR_COLOR) {
            return Some(RG_SCENE_HDR_COLOR);
        }
        None
    }

}
