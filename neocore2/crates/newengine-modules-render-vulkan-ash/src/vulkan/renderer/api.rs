use crate::error::VkResult;
use ash::vk;
use newengine_ui::draw::UiDrawList;

use super::state::VulkanRenderer;

impl VulkanRenderer {
    #[inline]
    #[allow(dead_code)]
    pub fn set_debug_text(&mut self, text: &str) {
        self.debug.debug_text.clear();
        self.debug.debug_text.push_str(text);
    }

    /// Resize request from the host. This is deferred and applied in begin_frame().
    pub fn resize(&mut self, width: u32, height: u32) -> VkResult<()> {
        if self.debug.target_width == width && self.debug.target_height == height {
            return Ok(());
        }

        self.set_target_size(width, height);

        // Defer swapchain recreation; it is expensive and must not be spammed during window drag.
        self.debug.swapchain_dirty = true;
        Ok(())
    }

    #[inline]
    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.debug.target_width = width;
        self.debug.target_height = height;
    }

    /// Stores UI draw list for the next presented frame.
    #[inline]
    pub fn set_ui_draw_list(&mut self, ui: UiDrawList) {
        self.debug.pending_ui = Some(ui);
    }

    /// Submits a short-lived upload command buffer using a persistent `UploadCtx`.
    ///
    /// This method does NOT call `queue_wait_idle`.
    /// It returns a fence that will be signaled once the upload work is complete.
    #[inline]
    #[allow(dead_code)]
    pub unsafe fn submit_upload<F: FnOnce(vk::CommandBuffer)>(
        &mut self,
        f: F,
    ) -> VkResult<vk::Fence> {
        let idx = self.frames.upload_cursor;
        self.frames.upload_cursor = (self.frames.upload_cursor + 1) % super::state::UPLOAD_CONTEXTS;

        let ctx = self.frames.upload_ctxs[idx];
        ctx.submit_async(&self.core.device, self.core.queue, f)
    }

    /// Schedules a staging buffer for destruction after `fence` is signaled.
    #[inline]
    #[allow(dead_code)]
    pub fn defer_free_staging_buffer(
        &mut self,
        fence: vk::Fence,
        buffer: vk::Buffer,
        memory: vk::DeviceMemory,
    ) {
        self.frames.deferred_free.push_buffer(fence, buffer, memory);
    }
}
