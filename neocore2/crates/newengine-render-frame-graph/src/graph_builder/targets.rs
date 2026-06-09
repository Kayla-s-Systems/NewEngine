use newengine_render_api::{
    Extent2D, RenderGraphResourceDesc, RenderGraphResourceId, RenderGraphResourceSemantic,
    RenderGraphResourceUsage, RenderTargetId, TextureFormat,
};

use super::FrameGraphBuilder;

pub const RG_SURFACE_COLOR: RenderGraphResourceId = RenderGraphResourceId(1);
pub const RG_VIEWPORT_COLOR: RenderGraphResourceId = RenderGraphResourceId(2);
pub const RG_VIEWPORT_DEPTH: RenderGraphResourceId = RenderGraphResourceId(3);
/// Linear scene color target. World shaders write lighting here without display encoding.
pub const RG_SCENE_HDR_COLOR: RenderGraphResourceId = RenderGraphResourceId(4);
pub const RG_SHADOW_MAP: RenderGraphResourceId = RenderGraphResourceId(10);
/// Companion fixed-function depth attachment for color-packed shadow visibility maps.
pub const RG_SHADOW_DEPTH: RenderGraphResourceId = RenderGraphResourceId(11);
pub const RG_GBUFFER_ALBEDO: RenderGraphResourceId = RenderGraphResourceId(20);
pub const RG_GBUFFER_NORMAL: RenderGraphResourceId = RenderGraphResourceId(21);
pub const RG_GBUFFER_MATERIAL: RenderGraphResourceId = RenderGraphResourceId(22);
pub const RG_GBUFFER_DEPTH: RenderGraphResourceId = RenderGraphResourceId(23);
pub const RG_LIT_COLOR: RenderGraphResourceId = RenderGraphResourceId(30);

#[derive(Debug, Clone, Copy)]
pub struct FrameGraphTargetDesc {
    pub surface_extent: Extent2D,
    pub viewport_extent: Extent2D,
    pub viewport_is_surface: bool,
    pub viewport_render_target: Option<RenderTargetId>,
    pub shadow_render_target: Option<RenderTargetId>,
    /// Final display/swapchain format. Keep LDR unless the platform exposes HDR swapchains.
    pub color_format: TextureFormat,
    /// Linear scene color format used by HDR-capable world/material shaders.
    pub scene_color_format: TextureFormat,
    pub depth_format: TextureFormat,
    pub hdr_scene_enabled: bool,
}

impl FrameGraphTargetDesc {
    #[inline]
    pub fn new(surface_extent: Extent2D, viewport_extent: Extent2D, viewport_is_surface: bool) -> Self {
        Self {
            surface_extent,
            viewport_extent,
            viewport_is_surface,
            viewport_render_target: None,
            shadow_render_target: None,
            color_format: TextureFormat::Bgra8Unorm,
            scene_color_format: TextureFormat::Rgba16Float,
            depth_format: TextureFormat::Depth32Float,
            hdr_scene_enabled: true,
        }
    }

    #[inline]
    pub fn with_viewport_render_target(mut self, target: Option<RenderTargetId>) -> Self {
        self.viewport_render_target = target;
        self
    }

    #[inline]
    pub fn with_shadow_render_target(mut self, target: Option<RenderTargetId>) -> Self {
        self.shadow_render_target = target;
        self
    }

    #[inline]
    pub fn with_hdr_scene(mut self, enabled: bool) -> Self {
        self.hdr_scene_enabled = enabled;
        self
    }

    #[inline]
    pub fn with_scene_color_format(mut self, format: TextureFormat) -> Self {
        self.scene_color_format = format;
        self
    }
}

impl FrameGraphBuilder {
    pub(super) fn add_standard_external_resources(&mut self) {
        self.graph.resources.push(RenderGraphResourceDesc::external_swapchain(
            RG_SURFACE_COLOR,
            "swapchain_surface_color",
            RenderGraphResourceUsage::ColorAttachment,
            self.target.surface_extent,
            self.target.color_format,
        )
        .with_semantic(RenderGraphResourceSemantic::SurfaceColor));

        if self.target.hdr_scene_enabled {
            self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
                RG_SCENE_HDR_COLOR,
                "scene_hdr_color",
                RenderGraphResourceUsage::ColorAttachment,
                self.target.viewport_extent,
                self.target.scene_color_format,
            )
            .with_semantic(RenderGraphResourceSemantic::SceneHdrColor));

            // HDR scene rendering is offscreen even when the viewport is the native surface.
            // The world pass must therefore own a matching depth attachment in the same
            // native render scope as scene_hdr_color. Binding RG_VIEWPORT_DEPTH to the
            // swapchain/external depth here makes the forward material pipelines use a
            // color+depth render pass while the actual offscreen target is color-only,
            // which can silently produce an empty scene before postFX composition.
            self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
                RG_VIEWPORT_DEPTH,
                "scene_hdr_depth",
                RenderGraphResourceUsage::DepthAttachment,
                self.target.viewport_extent,
                self.target.depth_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ViewportDepth));
        }

        if self.target.viewport_is_surface {
            self.graph.resources.push(RenderGraphResourceDesc::external_swapchain(
                RG_VIEWPORT_COLOR,
                "viewport_surface_color",
                RenderGraphResourceUsage::ColorAttachment,
                self.target.surface_extent,
                self.target.color_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ViewportColor));
            if !self.target.hdr_scene_enabled {
                self.graph.resources.push(RenderGraphResourceDesc::external_swapchain(
                    RG_VIEWPORT_DEPTH,
                    "viewport_surface_depth",
                    RenderGraphResourceUsage::DepthAttachment,
                    self.target.surface_extent,
                    self.target.depth_format,
                )
                .with_semantic(RenderGraphResourceSemantic::ViewportDepth));
            }
        } else if let Some(rt) = self.target.viewport_render_target {
            self.graph.resources.push(RenderGraphResourceDesc::external_render_target(
                RG_VIEWPORT_COLOR,
                "viewport_render_target_color",
                rt,
                RenderGraphResourceUsage::ColorAttachment,
                self.target.viewport_extent,
                self.target.color_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ViewportColor));
            if !self.target.hdr_scene_enabled {
                self.graph.resources.push(RenderGraphResourceDesc::external_render_target(
                    RG_VIEWPORT_DEPTH,
                    "viewport_render_target_depth",
                    rt,
                    RenderGraphResourceUsage::DepthAttachment,
                    self.target.viewport_extent,
                    self.target.depth_format,
                )
                .with_semantic(RenderGraphResourceSemantic::ViewportDepth));
            }
        } else {
            self.graph.resources.push(RenderGraphResourceDesc::external(
                RG_VIEWPORT_COLOR,
                "viewport_render_target_color",
                RenderGraphResourceUsage::ColorAttachment,
            )
            .with_semantic(RenderGraphResourceSemantic::ViewportColor));
            if !self.target.hdr_scene_enabled {
                self.graph.resources.push(RenderGraphResourceDesc::external(
                    RG_VIEWPORT_DEPTH,
                    "viewport_depth",
                    RenderGraphResourceUsage::DepthAttachment,
                )
                .with_semantic(RenderGraphResourceSemantic::ViewportDepth));
            }
        }
    }

}
