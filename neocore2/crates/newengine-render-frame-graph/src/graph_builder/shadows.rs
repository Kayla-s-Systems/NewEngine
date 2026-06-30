use newengine_render_api::{
    Extent2D, RenderGraphPassDomain, RenderGraphResourceDesc, RenderGraphResourceSemantic,
    RenderGraphResourceUsage, TextureFormat,
};

use crate::{DrawListKind, StandardRenderPhase};

use super::{FrameGraphBuilder, RG_SHADOW_DEPTH, RG_SHADOW_MAP};

impl FrameGraphBuilder {
    pub fn shadow_map(mut self, enabled: bool, resolution: u32) -> Self {
        if !enabled {
            return self;
        }
        let resolution = resolution.clamp(256, 8192);
        let shadow_extent = Extent2D::new(resolution, resolution);
        let shadow_resource = if let Some(rt) = self.target.shadow_render_target {
            RenderGraphResourceDesc::external_render_target(
                RG_SHADOW_MAP,
                "shadow_map",
                rt,
                RenderGraphResourceUsage::ColorAttachment,
                shadow_extent,
                TextureFormat::R32Float,
            )
            .with_semantic(RenderGraphResourceSemantic::ShadowMap)
        } else {
            RenderGraphResourceDesc::transient_texture(
                RG_SHADOW_MAP,
                "shadow_map",
                RenderGraphResourceUsage::ColorAttachment,
                shadow_extent,
                TextureFormat::R32Float,
            )
            .with_semantic(RenderGraphResourceSemantic::ShadowMap)
        };
        let external_shadow_rt = self.target.shadow_render_target.is_some();
        self.graph.resources.push(shadow_resource);
        if !external_shadow_rt {
            self.graph.resources.push(
                RenderGraphResourceDesc::transient_texture(
                    RG_SHADOW_DEPTH,
                    "shadow_depth_attachment",
                    RenderGraphResourceUsage::DepthAttachment,
                    shadow_extent,
                    self.target.depth_format,
                )
                .with_semantic(RenderGraphResourceSemantic::ViewportDepth),
            );
        }
        self.add_phase_pass(StandardRenderPhase::ShadowMap, |pass| {
            let pass = pass
                .with_domain(RenderGraphPassDomain::Render3d)
                .writes(RG_SHADOW_MAP, RenderGraphResourceUsage::ColorAttachment);
            let pass = if external_shadow_rt {
                pass
            } else {
                pass.writes(RG_SHADOW_DEPTH, RenderGraphResourceUsage::DepthAttachment)
            };
            pass.draw_list(DrawListKind::ShadowCasters)
        });
        self
    }

    #[inline]
    pub fn shadow_cascade_map(
        mut self,
        enabled: bool,
        resolution: u32,
        cascade_count: u32,
    ) -> Self {
        if !enabled {
            return self;
        }
        let resolution = resolution.clamp(256, 8192);
        let cascades = cascade_count.clamp(1, 8);
        let atlas_cols = if cascades <= 1 {
            1
        } else if cascades <= 4 {
            2
        } else {
            4
        };
        let atlas_rows = cascades.div_ceil(atlas_cols).max(1);
        let shadow_extent = Extent2D::new(
            resolution.saturating_mul(atlas_cols),
            resolution.saturating_mul(atlas_rows),
        );
        let shadow_resource = if let Some(rt) = self.target.shadow_render_target {
            RenderGraphResourceDesc::external_render_target(
                RG_SHADOW_MAP,
                "shadow_cascade_atlas",
                rt,
                RenderGraphResourceUsage::ColorAttachment,
                shadow_extent,
                TextureFormat::R32Float,
            )
            .with_semantic(RenderGraphResourceSemantic::ShadowMap)
        } else {
            RenderGraphResourceDesc::transient_texture(
                RG_SHADOW_MAP,
                "shadow_cascade_atlas",
                RenderGraphResourceUsage::ColorAttachment,
                shadow_extent,
                TextureFormat::R32Float,
            )
            .with_semantic(RenderGraphResourceSemantic::ShadowMap)
        };
        let external_shadow_rt = self.target.shadow_render_target.is_some();
        if !self.has_resource(RG_SHADOW_MAP) {
            self.graph.resources.push(shadow_resource);
        }
        if !external_shadow_rt && !self.has_resource(RG_SHADOW_DEPTH) {
            self.graph.resources.push(
                RenderGraphResourceDesc::transient_texture(
                    RG_SHADOW_DEPTH,
                    "shadow_cascade_depth_attachment",
                    RenderGraphResourceUsage::DepthAttachment,
                    shadow_extent,
                    self.target.depth_format,
                )
                .with_semantic(RenderGraphResourceSemantic::ViewportDepth),
            );
        }
        self.add_phase_pass(StandardRenderPhase::ShadowCascadeMap, |pass| {
            let pass = pass
                .with_domain(RenderGraphPassDomain::Render3d)
                .writes(RG_SHADOW_MAP, RenderGraphResourceUsage::ColorAttachment);
            let pass = if external_shadow_rt {
                pass
            } else {
                pass.writes(RG_SHADOW_DEPTH, RenderGraphResourceUsage::DepthAttachment)
            };
            pass.draw_list(DrawListKind::ShadowCasters)
        });
        self
    }
}
