use newengine_render_api::{RenderGraphPassDomain, RenderGraphResourceUsage};

use crate::{DrawListKind, StandardRenderPhase};

use super::{FrameGraphBuilder, RG_SURFACE_COLOR};

impl FrameGraphBuilder {
    #[inline]
    pub fn ui_backdrop_blur(mut self, enabled: bool) -> Self {
        if !enabled {
            return self;
        }
        let Some(input) = self.sampleable_scene_input_resource() else {
            return self;
        };
        self.add_phase_pass(StandardRenderPhase::UiBackdropBlur, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render2d)
                .reads(input, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn ui_composite(mut self, enabled: bool) -> Self {
        if !enabled {
            return self;
        }
        self.add_phase_pass(StandardRenderPhase::UiComposite, |pass| {
            // UI composition consumes UiLayerDrawPacket payloads attached to the
            // frame envelope. It is not a scene draw-list and must not recreate
            // the removed singleton UI draw-list compatibility funnel.
            pass.with_domain(RenderGraphPassDomain::Render2d)
                // UI is a post-scene presentation overlay: preserve the already resolved
                // surface color, then blend the retained UI domain into that same target.
                // Expressing the read explicitly prevents graph scheduling/lifetime analysis
                // from treating UI as a fresh scene color producer.
                .reads(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn debug_overlay(mut self, enabled: bool) -> Self {
        if enabled {
            self.add_phase_pass(StandardRenderPhase::DebugOverlay, |pass| {
                pass.with_domain(RenderGraphPassDomain::Render2d)
                    .reads(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
                    .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
                    .draw_list(DrawListKind::Debug)
            });
        }
        self
    }
}
