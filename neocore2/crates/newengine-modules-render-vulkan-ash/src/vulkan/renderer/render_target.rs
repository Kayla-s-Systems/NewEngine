use crate::error::{VkRenderError, VkResult};
use crate::vulkan::util::immediate_submit;

use ash::vk;

use super::state::{RenderTargetVk, VulkanRenderer};

#[inline]
unsafe fn find_memory_type(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    type_bits: u32,
    props: vk::MemoryPropertyFlags,
) -> Result<u32, VkRenderError> {
    let mem = instance.get_physical_device_memory_properties(physical_device);
    for i in 0..mem.memory_type_count {
        let mt = mem.memory_types[i as usize];
        let ok_bits = (type_bits & (1u32 << i)) != 0;
        let ok_flags = mt.property_flags.contains(props);
        if ok_bits && ok_flags {
            return Ok(i);
        }
    }
    Err(VkRenderError::AshWindow("No compatible memory type found".into()))
}

/// High-bit namespace fence for external UI textures.
///
/// Convention: `ui_tex_id = UI_EXTERNAL_BASE | render_target_id_u32`.
/// This avoids collisions with engine-managed UI texture ids.
pub(crate) const UI_EXTERNAL_BASE: u32 = 0x8000_0000;

#[inline]
pub(crate) const fn ui_external_id(render_target_id: u32) -> u32 {
    UI_EXTERNAL_BASE | (render_target_id & 0x7FFF_FFFF)
}

impl VulkanRenderer {
    pub fn create_render_target(&mut self, id: u32, extent: vk::Extent2D) -> VkResult<()> {
        unsafe {
            self.destroy_render_target(id);
        }

        if extent.width == 0 || extent.height == 0 {
            return Ok(());
        }

        let format = self.swapchain.format;

        unsafe {
            let image_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D {
                    width: extent.width,
                    height: extent.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::TRANSFER_SRC,
                )
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let image = self.core.device.create_image(&image_info, None)?;
            let req = self.core.device.get_image_memory_requirements(image);

            let mem_type = find_memory_type(
                &self.core.instance,
                self.core.physical_device,
                req.memory_type_bits,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;

            let alloc = vk::MemoryAllocateInfo::default()
                .allocation_size(req.size)
                .memory_type_index(mem_type);
            let mem = self.core.device.allocate_memory(&alloc, None)?;
            self.core.device.bind_image_memory(image, mem, 0)?;

            let view_info = vk::ImageViewCreateInfo::default()
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
                );

            let view = self.core.device.create_image_view(&view_info, None)?;

            let fb_info = vk::FramebufferCreateInfo::default()
                .render_pass(self.pipelines.render_pass)
                .attachments(std::slice::from_ref(&view))
                .width(extent.width)
                .height(extent.height)
                .layers(1);
            let framebuffer = self.core.device.create_framebuffer(&fb_info, None)?;

            // Initialize layout to SHADER_READ_ONLY so UI sampling is always valid.
            immediate_submit(
                &self.core.device,
                self.frames.upload_command_pool,
                self.core.queue,
                |cmd| {
                    crate::vulkan::util::transition_image(
                        &self.core.device,
                        cmd,
                        image,
                        vk::ImageLayout::UNDEFINED,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    );
                },
            )?;

            let mut rt = RenderTargetVk::default();
            rt.extent = extent;
            rt.format = format;
            rt.color.image = image;
            rt.color.memory = mem;
            rt.color.view = view;
            rt.color.sampler = vk::Sampler::null();
            rt.framebuffer = framebuffer;
            rt.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

            self.render_targets.insert(id, rt);

            // Expose as external UI texture (stable convention).
            let ui_id = ui_external_id(id);
            self.ui_register_external_texture(ui_id, view)?;
        }

        Ok(())
    }

    pub unsafe fn destroy_render_target(&mut self, id: u32) {
        let ui_id = ui_external_id(id);
        self.ui_unregister_external_texture(ui_id);

        let Some(mut rt) = self.render_targets.remove(&id) else {
            return;
        };

        if rt.framebuffer != vk::Framebuffer::null() {
            self.core.device.destroy_framebuffer(rt.framebuffer, None);
            rt.framebuffer = vk::Framebuffer::null();
        }

        rt.color.destroy(&self.core.device);
    }

    pub fn resize_render_target(&mut self, id: u32, extent: vk::Extent2D) -> VkResult<()> {
        let Some(existing) = self.render_targets.get(&id) else {
            return Err(VkRenderError::InvalidState("resize_render_target: unknown id"));
        };

        if existing.extent.width == extent.width && existing.extent.height == extent.height {
            return Ok(());
        }

        self.create_render_target(id, extent)
    }
}
