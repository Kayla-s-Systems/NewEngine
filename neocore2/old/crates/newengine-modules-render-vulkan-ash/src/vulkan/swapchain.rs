use crate::error::VkResult;

use ash::vk;
use ash::Device;

use super::pipeline::*;
use super::text::*;
use super::VulkanRenderer;

/// Creates a swapchain. If `old_swapchain` is not null, Vulkan may reuse resources internally.
pub(super) fn create_swapchain(
    swapchain_loader: &ash::khr::swapchain::Device,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    width: u32,
    height: u32,
    queue_family_index: u32,
    old_swapchain: vk::SwapchainKHR,
) -> VkResult<(vk::SwapchainKHR, Vec<vk::Image>, vk::Format, vk::Extent2D)> {
    let caps = unsafe {
        surface_loader.get_physical_device_surface_capabilities(physical_device, surface)
    }?;

    let formats = unsafe {
        surface_loader.get_physical_device_surface_formats(physical_device, surface)
    }?;

    let present_modes = unsafe {
        surface_loader.get_physical_device_surface_present_modes(physical_device, surface)
    }?;

    let surface_format = formats
        .iter()
        .cloned()
        .find(|f| f.format == vk::Format::B8G8R8A8_UNORM)
        .unwrap_or(formats[0]);

    let present_mode = present_modes
        .iter()
        .cloned()
        .find(|m| *m == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO);

    let extent = if caps.current_extent.width != u32::MAX {
        caps.current_extent
    } else {
        vk::Extent2D {
            width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
            height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
        }
    };

    let image_count = (caps.min_image_count + 1).min(if caps.max_image_count == 0 {
        u32::MAX
    } else {
        caps.max_image_count
    });

    let family_indices = [queue_family_index];

    let create_info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .queue_family_indices(&family_indices)
        .pre_transform(caps.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .old_swapchain(old_swapchain);

    let swapchain = unsafe { swapchain_loader.create_swapchain(&create_info, None)? };
    let images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };

    Ok((swapchain, images, surface_format.format, extent))
}

pub(super) fn create_image_views(
    device: &Device,
    images: &[vk::Image],
    format: vk::Format,
) -> VkResult<Vec<vk::ImageView>> {
    let mut views = Vec::with_capacity(images.len());
    for &image in images {
        let iv = unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1),
                    ),
                None,
            )?
        };
        views.push(iv);
    }
    Ok(views)
}

impl VulkanRenderer {
    /// Recreates swapchain and all swapchain-dependent resources.
    ///
    /// Safety: must be called only when no command buffers are executing that reference old resources.
    pub(super) unsafe fn recreate_swapchain(&mut self) -> VkResult<()> {
        if self.debug.target_width == 0 || self.debug.target_height == 0 {
            return Ok(());
        }

        let _ = self.core.device.device_wait_idle();

        for &fb in &self.swapchain.framebuffers {
            self.core.device.destroy_framebuffer(fb, None);
        }
        self.swapchain.framebuffers.clear();

        for &iv in &self.swapchain.image_views {
            self.core.device.destroy_image_view(iv, None);
        }
        self.swapchain.image_views.clear();

        let old_swapchain = self.swapchain.swapchain;

        let (new_swapchain, new_images, new_format, new_extent) = create_swapchain(
            &self.core.swapchain_loader,
            &self.core.surface_loader,
            self.core.surface,
            self.core.physical_device,
            self.debug.target_width,
            self.debug.target_height,
            self.core.queue_family_index,
            old_swapchain,
        )?;

        if old_swapchain != vk::SwapchainKHR::null() {
            self.core
                .swapchain_loader
                .destroy_swapchain(old_swapchain, None);
        }

        let new_image_views = create_image_views(&self.core.device, &new_images, new_format)?;
        let new_image_count = new_images.len();
        let format_changed = new_format != self.swapchain.format;

        if format_changed {
            if self.pipelines.tri_pipeline != vk::Pipeline::null() {
                self.core.device.destroy_pipeline(self.pipelines.tri_pipeline, None);
                self.pipelines.tri_pipeline = vk::Pipeline::null();
            }
            if self.pipelines.tri_pipeline_layout != vk::PipelineLayout::null() {
                self.core
                    .device
                    .destroy_pipeline_layout(self.pipelines.tri_pipeline_layout, None);
                self.pipelines.tri_pipeline_layout = vk::PipelineLayout::null();
            }

            if self.pipelines.text_pipeline != vk::Pipeline::null() {
                self.core
                    .device
                    .destroy_pipeline(self.pipelines.text_pipeline, None);
                self.pipelines.text_pipeline = vk::Pipeline::null();
            }
            if self.pipelines.text_pipeline_layout != vk::PipelineLayout::null() {
                self.core
                    .device
                    .destroy_pipeline_layout(self.pipelines.text_pipeline_layout, None);
                self.pipelines.text_pipeline_layout = vk::PipelineLayout::null();
            }

            if self.pipelines.ui_pipeline != vk::Pipeline::null() {
                self.core.device.destroy_pipeline(self.pipelines.ui_pipeline, None);
                self.pipelines.ui_pipeline = vk::Pipeline::null();
            }
            if self.pipelines.ui_pipeline_layout != vk::PipelineLayout::null() {
                self.core
                    .device
                    .destroy_pipeline_layout(self.pipelines.ui_pipeline_layout, None);
                self.pipelines.ui_pipeline_layout = vk::PipelineLayout::null();
            }

            if self.pipelines.render_pass != vk::RenderPass::null() {
                self.core.device.destroy_render_pass(self.pipelines.render_pass, None);
                self.pipelines.render_pass = vk::RenderPass::null();
            }

            self.swapchain.format = new_format;
            self.pipelines.render_pass = create_render_pass(&self.core.device, self.swapchain.format)?;

            let (pl, p) = create_pipeline(&self.core.device, self.pipelines.render_pass)?;
            self.pipelines.tri_pipeline_layout = pl;
            self.pipelines.tri_pipeline = p;

            if self.text.desc_set_layout != vk::DescriptorSetLayout::null() {
                let (tpl, tp) = create_text_pipeline(
                    &self.core.device,
                    self.pipelines.render_pass,
                    self.text.desc_set_layout,
                )?;
                self.pipelines.text_pipeline_layout = tpl;
                self.pipelines.text_pipeline = tp;
            }

            if self.ui.desc_set_layout != vk::DescriptorSetLayout::null() {
                let (upl, up) = super::ui::create_ui_pipeline(
                    &self.core.device,
                    self.pipelines.render_pass,
                    self.ui.desc_set_layout,
                )?;
                self.pipelines.ui_pipeline_layout = upl;
                self.pipelines.ui_pipeline = up;
            }
        } else {
            self.swapchain.format = new_format;
        }

        let new_framebuffers = create_framebuffers(
            &self.core.device,
            self.pipelines.render_pass,
            &new_image_views,
            new_extent,
        )?;

        if self.frames.command_pool != vk::CommandPool::null() && !self.frames.command_buffers.is_empty() {
            self.core
                .device
                .free_command_buffers(self.frames.command_pool, &self.frames.command_buffers);
        }

        self.frames.command_buffers = self.core.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(self.frames.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(new_image_count as u32),
        )?;

        self.swapchain.swapchain = new_swapchain;
        self.swapchain.images = new_images;
        self.swapchain.extent = new_extent;
        self.swapchain.image_views = new_image_views;
        self.swapchain.framebuffers = new_framebuffers;

        self.swapchain.image_layouts = vec![vk::ImageLayout::UNDEFINED; new_image_count];
        self.frames.images_in_flight = vec![vk::Fence::null(); new_image_count];

        Ok(())
    }
}
