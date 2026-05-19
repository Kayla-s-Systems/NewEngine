use newengine_render_api::{Extent2D, RenderTargetId, TextureFormat};

use crate::{
    DrawListDesc, FrameGraphBuilder, FrameGraphTargetDesc, FramePlanExecutionMode,
    RenderFramePlan, RenderFrameRecipe, RuntimeFrameFeatureSet, RuntimeRecipeBuildParams,
};

#[derive(Debug, Clone)]
pub struct StandardRuntimePipelineDesc {
    pub frame_index: u64,
    pub surface_extent: Extent2D,
    pub viewport_extent: Extent2D,
    pub viewport_is_surface: bool,
    pub viewport_render_target: Option<RenderTargetId>,
    pub shadow_render_target: Option<RenderTargetId>,
    pub shadow_enabled: bool,
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
            shadow_enabled: true,
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
    target.scene_color_format = TextureFormat::Rgba16Float;
    target.depth_format = TextureFormat::Depth32Float;
    target.hdr_scene_enabled = desc.hdr_scene_enabled;
    target.viewport_render_target = desc.viewport_render_target;
    target.shadow_render_target = desc.shadow_render_target;

    let features = if desc.deferred {
        RuntimeFrameFeatureSet::deferred(
            desc.shadow_enabled,
            desc.postfx_enabled,
            desc.ui_enabled,
            desc.debug_overlay_enabled,
        ).with_ui_backdrop_blur(desc.ui_backdrop_blur_enabled)
    } else {
        RuntimeFrameFeatureSet::forward(
            desc.shadow_enabled,
            desc.postfx_enabled,
            desc.ui_enabled,
            desc.debug_overlay_enabled,
        ).with_ui_backdrop_blur(desc.ui_backdrop_blur_enabled)
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
