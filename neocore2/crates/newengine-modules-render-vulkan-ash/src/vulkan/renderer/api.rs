use crate::error::VkResult;
use newengine_ui::draw::UiDrawList;

use super::state::VulkanRenderer;

impl VulkanRenderer {
    #[inline]
    pub fn set_debug_text(&mut self, text: &str) {
        self.debug.debug_text.clear();
        self.debug.debug_text.push_str(text);
    }

    pub fn resize(&mut self, width: u32, height: u32) -> VkResult<()> {
        self.set_target_size(width, height);
        unsafe { self.recreate_swapchain() }
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
}
