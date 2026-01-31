use newengine_core::render::{BeginFrameDesc, RenderApi};
use newengine_core::{EngineError, EngineResult};

use crate::vulkan::VulkanRenderer;

pub struct VulkanRenderApi {
    renderer: VulkanRenderer,
}

impl VulkanRenderApi {
    #[inline]
    pub fn new(renderer: VulkanRenderer) -> Self {
        Self { renderer }
    }
}

impl RenderApi for VulkanRenderApi {
    fn begin_frame(&mut self, desc: BeginFrameDesc) -> EngineResult<()> {
        self.renderer
            .draw_clear_color(desc.clear_color)
            .map_err(|e| EngineError::other(e.to_string()))
    }

    fn end_frame(&mut self) -> EngineResult<()> {
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> EngineResult<()> {
        self.renderer.set_target_size(width, height);
        Ok(())
    }
}