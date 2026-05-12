use newengine_render_api::{Extent2D, RenderTargetId, TextureFormat};

use crate::{DrawListDesc, FrameGraphBuilder, FrameGraphTargetDesc, FramePlanExecutionMode, RenderFramePlan};

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
    pub deferred: bool,
    pub postfx_enabled: bool,
    pub ui_enabled: bool,
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
            deferred: false,
            postfx_enabled: false,
            ui_enabled: true,
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
    pub fn deferred(mut self, value: bool) -> Self {
        self.deferred = value;
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
    target.depth_format = TextureFormat::Depth32Float;
    target.viewport_render_target = desc.viewport_render_target;
    target.shadow_render_target = desc.shadow_render_target;

    FrameGraphBuilder::new("runtime.standard_frame", desc.frame_index, target)
        .execution_mode(desc.execution_mode)
        .draw_lists(desc.draw_lists)
        .shadow_map(desc.shadow_enabled, desc.shadow_resolution)
        .viewport_gbuffer_or_forward(desc.deferred)
        .lighting(desc.deferred)
        .postfx(desc.postfx_enabled)
        .ui_composite(desc.ui_enabled)
        .debug_overlay(desc.debug_overlay_enabled)
        .submit()
}
