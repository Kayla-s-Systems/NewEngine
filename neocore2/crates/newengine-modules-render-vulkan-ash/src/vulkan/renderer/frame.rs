use crate::error::{VkRenderError, VkResult};
use crate::vulkan::util::transition_image;

use ash::vk;

use super::state::VulkanRenderer;
use super::types::FRAMES_IN_FLIGHT;

impl VulkanRenderer {
    pub fn begin_frame(&mut self, clear_rgba: [f32; 4]) -> VkResult<()> {
        if self.debug.in_frame {
            return Err(VkRenderError::InvalidState("begin_frame called while already in frame"));
        }

        // If window is minimized or has no drawable area: keep state clean and do nothing.
        if self.debug.target_width == 0 || self.debug.target_height == 0 {
            self.debug.swapchain_dirty = true;
            return Ok(());
        }

        // Apply deferred swapchain recreation exactly once at a safe point.
        if self.debug.swapchain_dirty {
            self.debug.swapchain_dirty = false;
            unsafe { self.recreate_swapchain()? };
        }

        let frame = self.frames.frames[self.frames.frame_index];

        unsafe {
            self.core
                .device
                .wait_for_fences(&[frame.in_flight], true, u64::MAX)?;
        }

        let (image_index, _suboptimal) = match unsafe {
            self.core.swapchain_loader.acquire_next_image(
                self.swapchain.swapchain,
                u64::MAX,
                frame.image_available,
                vk::Fence::null(),
            )
        } {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                self.debug.swapchain_dirty = true;
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };

        let idx = image_index as usize;

        unsafe {
            let inflight = self.frames.images_in_flight[idx];
            if inflight != vk::Fence::null() {
                self.core
                    .device
                    .wait_for_fences(&[inflight], true, u64::MAX)?;
            }
            self.frames.images_in_flight[idx] = frame.in_flight;
            self.core.device.reset_fences(&[frame.in_flight])?;
        }

        let cmd = self.frames.command_buffers[idx];
        let image = self.swapchain.images[idx];

        unsafe {
            self.core
                .device
                .reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;

            self.core.device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;

            let old_layout = self.swapchain.image_layouts[idx];
            transition_image(
                &self.core.device,
                cmd,
                image,
                old_layout,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );

            let clear = vk::ClearValue {
                color: vk::ClearColorValue { float32: clear_rgba },
            };

            let rp_begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.pipelines.render_pass)
                .framebuffer(self.swapchain.framebuffers[idx])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain.extent,
                })
                .clear_values(std::slice::from_ref(&clear));

            self.core
                .device
                .cmd_begin_render_pass(cmd, &rp_begin, vk::SubpassContents::INLINE);

            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: self.swapchain.extent.width as f32,
                height: self.swapchain.extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.swapchain.extent,
            };

            self.core
                .device
                .cmd_set_viewport(cmd, 0, std::slice::from_ref(&viewport));
            self.core
                .device
                .cmd_set_scissor(cmd, 0, std::slice::from_ref(&scissor));
        }

        self.debug.in_frame = true;
        self.debug.current_image_index = image_index;
        self.debug.current_swapchain_idx = idx;
        Ok(())
    }

    pub fn end_frame(&mut self) -> VkResult<()> {
        if !self.debug.in_frame {
            return Err(VkRenderError::InvalidState("end_frame called without begin_frame"));
        }

        let frame = self.frames.frames[self.frames.frame_index];
        let idx = self.debug.current_swapchain_idx;
        let cmd = self.frames.command_buffers[idx];
        let image = self.swapchain.images[idx];
        let image_index = self.debug.current_image_index;

        unsafe {
            if self.pipelines.text_pipeline != vk::Pipeline::null()
                && self.pipelines.text_pipeline_layout != vk::PipelineLayout::null()
                && !self.debug.debug_text.is_empty()
            {
                let debug_text = std::mem::take(&mut self.debug.debug_text);
                let res = self.draw_text_overlay(cmd, &debug_text);
                self.debug.debug_text = debug_text;
                res?;
            }

            if let Some(list) = self.debug.pending_ui.take() {
                let ui_ready = self.pipelines.ui_pipeline != vk::Pipeline::null()
                    && self.pipelines.ui_pipeline_layout != vk::PipelineLayout::null()
                    && self.ui.desc_set_layout != vk::DescriptorSetLayout::null()
                    && self.ui.sampler != vk::Sampler::null();

                if ui_ready {
                    self.ui_upload_and_draw(cmd, &list)?;
                }
            }

            self.core.device.cmd_end_render_pass(cmd);

            transition_image(
                &self.core.device,
                cmd,
                image,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );
            self.swapchain.image_layouts[idx] = vk::ImageLayout::PRESENT_SRC_KHR;

            self.core.device.end_command_buffer(cmd)?;

            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let wait_sems = [frame.image_available];
            let signal_sems = [frame.render_finished];
            let cmd_bufs = [cmd];

            let submit_infos = [vk::SubmitInfo::default()
                .wait_semaphores(&wait_sems)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&cmd_bufs)
                .signal_semaphores(&signal_sems)];

            self.core
                .device
                .queue_submit(self.core.queue, &submit_infos, frame.in_flight)?;

            let swapchains = [self.swapchain.swapchain];
            let indices = [image_index];

            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal_sems)
                .swapchains(&swapchains)
                .image_indices(&indices);

            match self
                .core
                .swapchain_loader
                .queue_present(self.core.queue, &present_info)
            {
                Ok(_) => {}
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                    self.debug.swapchain_dirty = true;
                }
                Err(e) => return Err(e.into()),
            }
        }

        self.frames.frame_index = (self.frames.frame_index + 1) % FRAMES_IN_FLIGHT;
        self.debug.in_frame = false;
        Ok(())
    }
}