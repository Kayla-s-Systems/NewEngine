use newengine_render_api::{Extent2D, RenderTargetId, TextureFormat};

use crate::{
    DrawListDesc, FrameGraphBuilder, FrameGraphTargetDesc, FramePlanExecutionMode, RenderFramePlan,
    RenderFrameRecipe, RuntimeFrameFeatureSet, RuntimeRecipeBuildParams,
};

#[derive(Debug, Clone)]
pub struct StandardRuntimePipelineDesc {
    pub frame_index: u64,
    pub surface_extent: Extent2D,
    pub viewport_extent: Extent2D,
    pub viewport_is_surface: bool,
    pub viewport_render_target: Option<RenderTargetId>,
    pub shadow_render_target: Option<RenderTargetId>,
    pub local_shadow_render_target: Option<RenderTargetId>,
    pub local_shadow_extent: Extent2D,
    pub shadow_enabled: bool,
    pub local_shadow_enabled: bool,
    pub shadow_resolution: u32,
    pub shadow_cascade_count: u32,
    pub deferred: bool,
    pub hdr_scene_enabled: bool,
    pub postfx_enabled: bool,
    pub ui_enabled: bool,
    pub ui_backdrop_blur_enabled: bool,
    pub debug_overlay_enabled: bool,
    pub execution_mode: FramePlanExecutionMode,
    pub draw_lists: Vec<DrawListDesc>,
}

impl StandardRuntimePipelineDesc {
    #[inline]
    pub fn new(frame_index: u64, surface_extent: Extent2D, viewport_extent: Extent2D) -> Self {
        Self {
            frame_index,
            surface_extent,
            viewport_extent,
            viewport_is_surface: false,
            viewport_render_target: None,
            shadow_render_target: None,
            local_shadow_render_target: None,
            local_shadow_extent: Extent2D::new(1, 1),
            shadow_enabled: true,
            local_shadow_enabled: false,
            shadow_resolution: 2048,
            shadow_cascade_count: 1,
            deferred: false,
            hdr_scene_enabled: true,
            postfx_enabled: true,
            ui_enabled: true,
            ui_backdrop_blur_enabled: false,
            debug_overlay_enabled: true,
            execution_mode: FramePlanExecutionMode::ImmediateCallbacks,
            draw_lists: Vec::new(),
        }
    }

    #[inline]
    pub fn viewport_is_surface(mut self, value: bool) -> Self {
        self.viewport_is_surface = value;
        self
    }

    #[inline]
    pub fn viewport_render_target(mut self, target: Option<RenderTargetId>) -> Self {
        self.viewport_render_target = target;
        self
    }

    #[inline]
    pub fn shadow_render_target(mut self, target: Option<RenderTargetId>) -> Self {
        self.shadow_render_target = target;
        self
    }

    #[inline]
    pub fn local_shadow(
        mut self,
        enabled: bool,
        target: Option<RenderTargetId>,
        extent: Extent2D,
    ) -> Self {
        self.local_shadow_enabled = enabled;
        self.local_shadow_render_target = target;
        self.local_shadow_extent = extent;
        self
    }

    #[inline]
    pub fn shadow(mut self, enabled: bool, resolution: u32) -> Self {
        self.shadow_enabled = enabled;
        self.shadow_resolution = resolution;
        self
    }

    #[inline]
    pub fn shadow_cascades(mut self, cascade_count: u32) -> Self {
        self.shadow_cascade_count = cascade_count.clamp(1, 8);
        self
    }

    #[inline]
    pub fn deferred(mut self, value: bool) -> Self {
        self.deferred = value;
        self
    }

    #[inline]
    pub fn hdr_scene(mut self, enabled: bool) -> Self {
        self.hdr_scene_enabled = enabled;
        self
    }

    #[inline]
    pub fn postfx(mut self, enabled: bool) -> Self {
        self.postfx_enabled = enabled;
        self
    }

    #[inline]
    pub fn ui(mut self, enabled: bool) -> Self {
        self.ui_enabled = enabled;
        self
    }

    #[inline]
    pub fn ui_backdrop_blur(mut self, enabled: bool) -> Self {
        self.ui_backdrop_blur_enabled = enabled;
        self
    }

    #[inline]
    pub fn debug_overlay(mut self, enabled: bool) -> Self {
        self.debug_overlay_enabled = enabled;
        self
    }

    #[inline]
    pub fn draw_lists(mut self, lists: impl IntoIterator<Item = DrawListDesc>) -> Self {
        self.draw_lists = lists.into_iter().collect();
        self
    }
}

#[inline]
pub fn standard_runtime_frame(desc: StandardRuntimePipelineDesc) -> RenderFramePlan {
    let mut target = FrameGraphTargetDesc::new(
        desc.surface_extent,
        desc.viewport_extent,
        desc.viewport_is_surface,
    );
    target.color_format = TextureFormat::Bgra8Unorm;
    target.scene_color_format = if desc.hdr_scene_enabled {
        TextureFormat::Rgba16Float
    } else {
        TextureFormat::Bgra8Unorm
    };
    target.depth_format = TextureFormat::Depth32Float;
    target.hdr_scene_enabled = desc.hdr_scene_enabled;
    target.offscreen_scene_enabled = desc.hdr_scene_enabled || desc.postfx_enabled;
    target.viewport_render_target = desc.viewport_render_target;
    target.shadow_render_target = desc.shadow_render_target;
    target.local_shadow_render_target = desc.local_shadow_render_target;
    target.local_shadow_extent = desc.local_shadow_extent;

    let features = if desc.deferred {
        RuntimeFrameFeatureSet::deferred(
            desc.shadow_enabled,
            desc.postfx_enabled,
            desc.ui_enabled,
            desc.debug_overlay_enabled,
        )
        .with_ui_backdrop_blur(desc.ui_backdrop_blur_enabled)
        .with_local_shadows(desc.local_shadow_enabled)
    } else {
        RuntimeFrameFeatureSet::forward(
            desc.shadow_enabled,
            desc.postfx_enabled,
            desc.ui_enabled,
            desc.debug_overlay_enabled,
        )
        .with_ui_backdrop_blur(desc.ui_backdrop_blur_enabled)
        .with_local_shadows(desc.local_shadow_enabled)
    };
    let recipe = RenderFrameRecipe::standard_runtime_with_shadow_mode(
        features,
        desc.shadow_cascade_count > 1,
    );
    let label = recipe.label.clone();

    FrameGraphBuilder::new(label, desc.frame_index, target)
        .execution_mode(desc.execution_mode)
        .draw_lists(desc.draw_lists)
        .apply_runtime_recipe(
            &recipe,
            RuntimeRecipeBuildParams::new(desc.shadow_resolution)
                .with_shadow_cascade_count(desc.shadow_cascade_count),
        )
        .submit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_render_api::RenderGraphResourceSemantic;

    #[test]
    fn ldr_standard_pipeline_uses_display_compatible_scene_color() {
        let plan = standard_runtime_frame(
            StandardRuntimePipelineDesc::new(1, Extent2D::new(1600, 900), Extent2D::new(488, 236))
                .viewport_render_target(Some(RenderTargetId(
                    std::num::NonZeroU32::new(77).unwrap(),
                )))
                .hdr_scene(false)
                .postfx(false)
                .shadow(false, 1),
        );
        let viewport = plan
            .graph
            .resources
            .iter()
            .find(|resource| resource.semantic == RenderGraphResourceSemantic::ViewportColor)
            .expect("viewport color resource");
        assert_eq!(viewport.format, Some(TextureFormat::Bgra8Unorm));
        assert!(!plan
            .graph
            .resources
            .iter()
            .any(|resource| { resource.semantic == RenderGraphResourceSemantic::SceneHdrColor }));
    }

    #[test]
    fn ldr_postfx_uses_sampleable_bgra_scene_color_and_depth() {
        let plan = standard_runtime_frame(
            StandardRuntimePipelineDesc::new(2, Extent2D::new(1600, 900), Extent2D::new(1600, 900))
                .viewport_is_surface(true)
                .hdr_scene(false)
                .postfx(true)
                .ui(false)
                .debug_overlay(false),
        );
        let scene = plan
            .graph
            .resources
            .iter()
            .find(|resource| resource.semantic == RenderGraphResourceSemantic::SceneHdrColor)
            .expect("LDR postFX still requires a sampleable offscreen scene color");
        assert_eq!(scene.format, Some(TextureFormat::Bgra8Unorm));
        assert!(plan.graph.resources.iter().any(|resource| {
            resource.semantic == RenderGraphResourceSemantic::ViewportDepth
                && resource.lifetime
                    == newengine_render_api::RenderGraphResourceLifetime::TransientFrame
        }));
        let forward = plan
            .graph
            .passes
            .iter()
            .find(|pass| pass.kind == newengine_render_api::RenderGraphPassKind::ForwardOpaque)
            .expect("forward pass");
        assert!(forward
            .writes
            .iter()
            .any(|write| write.resource == crate::RG_SCENE_HDR_COLOR));
    }

    #[test]
    fn hdr_standard_pipeline_keeps_float_scene_color() {
        let plan = standard_runtime_frame(StandardRuntimePipelineDesc::new(
            1,
            Extent2D::new(1600, 900),
            Extent2D::new(488, 236),
        ));
        let scene = plan
            .graph
            .resources
            .iter()
            .find(|resource| resource.semantic == RenderGraphResourceSemantic::SceneHdrColor)
            .expect("HDR scene color resource");
        assert_eq!(scene.format, Some(TextureFormat::Rgba16Float));
    }
}
