use crate::error::VkResult;

use ash::vk;
use ash::Device;

use super::pipeline::*;
use super::text::*;
use super::VulkanRenderer;

pub(super) fn create_swapchain(
    swapchain_loader: &ash::khr::swapchain::Device,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    width: u32,
    height: u32,
    queue_family_index: u32,
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

    let image_count = (caps.min_image_count + 1).min(
        if caps.max_image_count == 0 {
            u32::MAX
        } else {
            caps.max_image_count
        },
    );

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
        .old_swapchain(vk::SwapchainKHR::null());

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
    pub(super) unsafe fn recreate_swapchain(&mut self) -> VkResult<()> {
        if self.target_width == 0 || self.target_height == 0 {
            return Ok(());
        }

        let _ = self.device.device_wait_idle();

        for &fb in &self.framebuffers {
            self.device.destroy_framebuffer(fb, None);
        }
        self.framebuffers.clear();

        for &iv in &self.swapchain_image_views {
            self.device.destroy_image_view(iv, None);
        }
        self.swapchain_image_views.clear();

        self.swapchain_loader
            .destroy_swapchain(self.swapchain, None);

        let (swapchain, swapchain_images, swapchain_format, extent) = create_swapchain(
            &self.swapchain_loader,
            &self.surface_loader,
            self.surface,
            self.physical_device,
            self.target_width,
            self.target_height,
            self.queue_family_index,
        )?;

        let swapchain_image_views =
            create_image_views(&self.device, &swapchain_images, swapchain_format)?;

        let format_changed = swapchain_format != self.swapchain_format;

        if format_changed {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);

            self.device.destroy_pipeline(self.text_pipeline, None);
            self.device.destroy_pipeline_layout(self.text_pipeline_layout, None);

            self.device.destroy_render_pass(self.render_pass, None);

            self.swapchain_format = swapchain_format;
            self.render_pass = create_render_pass(&self.device, self.swapchain_format)?;

            let (pl, p) = create_pipeline(&self.device, self.render_pass)?;
            self.pipeline_layout = pl;
            self.pipeline = p;

            let (tpl, tp) = create_text_pipeline(
                &self.device,
                self.render_pass,
                self.text_desc_set_layout,
            )?;
            self.text_pipeline_layout = tpl;
            self.text_pipeline = tp;
        } else {
            self.swapchain_format = swapchain_format;
        }

        let framebuffers =
            create_framebuffers(&self.device, self.render_pass, &swapchain_image_views, extent)?;

        self.device
            .free_command_buffers(self.command_pool, &self.command_buffers);

        self.command_buffers = self.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(self.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(swapchain_images.len() as u32),
        )?;

        self.swapchain = swapchain;
        self.swapchain_images = swapchain_images;
        self.extent = extent;
        self.swapchain_image_views = swapchain_image_views;
        self.framebuffers = framebuffers;
        self.image_layouts = vec![vk::ImageLayout::UNDEFINED; self.swapchain_images.len()];

        Ok(())
    }
}