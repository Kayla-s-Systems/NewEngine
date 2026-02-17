use crate::error::{VkRenderError, VkResult};
use crate::vulkan::util::immediate_submit;

use ash::vk;

use newengine_ui::texture::reserved as ui_reserved;

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

#[inline]
fn ui_external_id(render_target_id: u32) -> u32 {
    ui_reserved::external_from_u32(render_target_id).0
}

impl VulkanRenderer {
    pub fn create_render_target(&mut self, id: u32, extent: vk::Extent2D, with_depth: bool) -> VkResult<()> {
        unsafe {
            self.destroy_render_target(id);
        }

        if extent.width == 0 || extent.height == 0 {
            return Ok(());
        }

        let format = self.swapchain.format;
        let depth_format = vk::Format::D32_SFLOAT;

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

            let (framebuffer, depth_alloc) = if with_depth {
                let depth_info = vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(depth_format)
                    .extent(vk::Extent3D {
                        width: extent.width,
                        height: extent.height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED);

                let depth_image = self.core.device.create_image(&depth_info, None)?;
                let depth_req = self.core.device.get_image_memory_requirements(depth_image);
                let depth_mem_type = find_memory_type(
                    &self.core.instance,
                    self.core.physical_device,
                    depth_req.memory_type_bits,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )?;

                let depth_alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(depth_req.size)
                    .memory_type_index(depth_mem_type);
                let depth_mem = self.core.device.allocate_memory(&depth_alloc_info, None)?;
                self.core.device.bind_image_memory(depth_image, depth_mem, 0)?;

                let depth_view_info = vk::ImageViewCreateInfo::default()
                    .image(depth_image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(depth_format)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1),
                    );
                let depth_view = self.core.device.create_image_view(&depth_view_info, None)?;

                let attachments = [view, depth_view];
                let fb_info = vk::FramebufferCreateInfo::default()
                    .render_pass(self.pipelines.render_pass_depth)
                    .attachments(&attachments)
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                let framebuffer = self.core.device.create_framebuffer(&fb_info, None)?;

                (framebuffer, Some((depth_image, depth_mem, depth_view)))
            } else {
                let fb_info = vk::FramebufferCreateInfo::default()
                    .render_pass(self.pipelines.render_pass)
                    .attachments(std::slice::from_ref(&view))
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                let framebuffer = self.core.device.create_framebuffer(&fb_info, None)?;
                (framebuffer, None)
            };

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

            rt.has_depth = with_depth;
            rt.depth_format = depth_format;
            rt.depth_layout = vk::ImageLayout::UNDEFINED;
            if let Some((di, dm, dv)) = depth_alloc {
                rt.depth.image = di;
                rt.depth.memory = dm;
                rt.depth.view = dv;
                rt.depth.sampler = vk::Sampler::null();
            }

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

        if rt.has_depth {
            rt.depth.destroy(&self.core.device);
        }
        rt.color.destroy(&self.core.device);
    }

    #[allow(dead_code)]
    pub fn resize_render_target(&mut self, id: u32, extent: vk::Extent2D) -> VkResult<()> {
        let Some(existing) = self.render_targets.get(&id) else {
            return Err(VkRenderError::InvalidState("resize_render_target: unknown id"));
        };

        if existing.extent.width == extent.width && existing.extent.height == extent.height {
            return Ok(());
        }

        self.create_render_target(id, extent, existing.has_depth)
    }
}
