use ash::vk;

use super::state::VulkanRenderer;

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.core.device.device_wait_idle();

            self.destroy_ui_overlay();
            self.destroy_text_overlay();

            if self.frames.upload_command_pool != vk::CommandPool::null() {
                self.core
                    .device
                    .destroy_command_pool(self.frames.upload_command_pool, None);
                self.frames.upload_command_pool = vk::CommandPool::null();
            }

            for f in &self.frames.frames {
                if f.in_flight != vk::Fence::null() {
                    self.core.device.destroy_fence(f.in_flight, None);
                }
                if f.render_finished != vk::Semaphore::null() {
                    self.core.device.destroy_semaphore(f.render_finished, None);
                }
                if f.image_available != vk::Semaphore::null() {
                    self.core.device.destroy_semaphore(f.image_available, None);
                }
            }

            if self.frames.command_pool != vk::CommandPool::null() {
                if !self.frames.command_buffers.is_empty() {
                    self.core.device.free_command_buffers(
                        self.frames.command_pool,
                        &self.frames.command_buffers,
                    );
                }
                self.core
                    .device
                    .destroy_command_pool(self.frames.command_pool, None);
                self.frames.command_pool = vk::CommandPool::null();
            }

            for &fb in &self.swapchain.framebuffers {
                if fb != vk::Framebuffer::null() {
                    self.core.device.destroy_framebuffer(fb, None);
                }
            }
            self.swapchain.framebuffers.clear();

            if self.pipelines.tri_pipeline != vk::Pipeline::null() {
                self.core
                    .device
                    .destroy_pipeline(self.pipelines.tri_pipeline, None);
                self.pipelines.tri_pipeline = vk::Pipeline::null();
            }
            if self.pipelines.tri_pipeline_layout != vk::PipelineLayout::null() {
                self.core
                    .device
                    .destroy_pipeline_layout(self.pipelines.tri_pipeline_layout, None);
                self.pipelines.tri_pipeline_layout = vk::PipelineLayout::null();
            }

            if self.pipelines.render_pass != vk::RenderPass::null() {
                self.core
                    .device
                    .destroy_render_pass(self.pipelines.render_pass, None);
                self.pipelines.render_pass = vk::RenderPass::null();
            }

            for &iv in &self.swapchain.image_views {
                if iv != vk::ImageView::null() {
                    self.core.device.destroy_image_view(iv, None);
                }
            }
            self.swapchain.image_views.clear();

            if self.swapchain.swapchain != vk::SwapchainKHR::null() {
                self.core
                    .swapchain_loader
                    .destroy_swapchain(self.swapchain.swapchain, None);
                self.swapchain.swapchain = vk::SwapchainKHR::null();
            }

            if self.core.surface != vk::SurfaceKHR::null() {
                self.core
                    .surface_loader
                    .destroy_surface(self.core.surface, None);
                self.core.surface = vk::SurfaceKHR::null();
            }

            self.core.device.destroy_device(None);
            self.core.instance.destroy_instance(None);
        }
    }
}
