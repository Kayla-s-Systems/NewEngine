use crate::error::VkResult;

use ash::vk;
use std::mem;
use std::ptr;

use super::device::*;
use super::pipeline::create_shader_module;
use super::util::*;
use super::VulkanRenderer;

mod font8x8 {
    include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/vulkan/font8x8_ascii.inl"
    ));
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct TextVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

impl TextVertex {
    #[inline]
    fn new(pos: [f32; 2], uv: [f32; 2], color: [f32; 4]) -> Self {
        Self { pos, uv, color }
    }
}

pub(super) unsafe fn create_text_pipeline(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
) -> VkResult<(vk::PipelineLayout, vk::Pipeline)> {
    let vert = create_shader_module(
        device,
        include_bytes!(concat!(env!("OUT_DIR"), "/text.vert.spv")),
    )?;
    let frag = create_shader_module(
        device,
        include_bytes!(concat!(env!("OUT_DIR"), "/text.frag.spv")),
    )?;

    let entry = std::ffi::CString::new("main").unwrap();

    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert)
            .name(&entry),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag)
            .name(&entry),
    ];

    let binding = vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(mem::size_of::<TextVertex>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX);

    let attrs = [
        vk::VertexInputAttributeDescription::default()
            .location(0)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0),
        vk::VertexInputAttributeDescription::default()
            .location(1)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(8),
        vk::VertexInputAttributeDescription::default()
            .location(2)
            .binding(0)
            .format(vk::Format::R32G32B32A32_SFLOAT)
            .offset(16),
    ];

    let vi = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(std::slice::from_ref(&binding))
        .vertex_attribute_descriptions(&attrs);

    let ia = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    let vp = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);

    let rs = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0);

    let ms = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);

    let ca = vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD)
        .color_write_mask(
            vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A,
        );

    let cb = vk::PipelineColorBlendStateCreateInfo::default()
        .attachments(std::slice::from_ref(&ca));

    let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let ds = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);

    let set_layouts = [set_layout];
    let layout = device.create_pipeline_layout(
        &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts),
        None,
    )?;

    let gp = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vi)
        .input_assembly_state(&ia)
        .viewport_state(&vp)
        .rasterization_state(&rs)
        .multisample_state(&ms)
        .color_blend_state(&cb)
        .dynamic_state(&ds)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0);

    let pipelines = device.create_graphics_pipelines(vk::PipelineCache::null(), &[gp], None);
    let pipeline = match pipelines {
        Ok(v) => v[0],
        Err((_, e)) => return Err(e.into()),
    };

    device.destroy_shader_module(vert, None);
    device.destroy_shader_module(frag, None);

    Ok((layout, pipeline))
}

impl VulkanRenderer {
    pub(super) fn init_text_overlay(&mut self) -> VkResult<()> {
        unsafe {
            let atlas = build_font_atlas_128_r8();
            debug_assert_eq!(atlas.len(), 128 * 128);

            self.create_font_resources(&atlas)?;
            self.create_text_descriptor()?;

            let (tpl, tp) = create_text_pipeline(
                &self.device,
                self.render_pass,
                self.text_desc_set_layout,
            )?;
            self.text_pipeline_layout = tpl;
            self.text_pipeline = tp;

            self.create_text_vertex_buffer(6 * 2048)?;
        }
        Ok(())
    }

    pub(super) unsafe fn destroy_text_overlay(&mut self) {
        if self.text_vb != vk::Buffer::null() {
            self.device.destroy_buffer(self.text_vb, None);
        }
        if self.text_vb_mem != vk::DeviceMemory::null() {
            self.device.free_memory(self.text_vb_mem, None);
        }

        if self.text_pipeline != vk::Pipeline::null() {
            self.device.destroy_pipeline(self.text_pipeline, None);
        }
        if self.text_pipeline_layout != vk::PipelineLayout::null() {
            self.device
                .destroy_pipeline_layout(self.text_pipeline_layout, None);
        }

        if self.text_desc_pool != vk::DescriptorPool::null() {
            self.device.destroy_descriptor_pool(self.text_desc_pool, None);
        }
        if self.text_desc_set_layout != vk::DescriptorSetLayout::null() {
            self.device
                .destroy_descriptor_set_layout(self.text_desc_set_layout, None);
        }

        if self.font_sampler != vk::Sampler::null() {
            self.device.destroy_sampler(self.font_sampler, None);
        }
        if self.font_image_view != vk::ImageView::null() {
            self.device.destroy_image_view(self.font_image_view, None);
        }
        if self.font_image != vk::Image::null() {
            self.device.destroy_image(self.font_image, None);
        }
        if self.font_image_mem != vk::DeviceMemory::null() {
            self.device.free_memory(self.font_image_mem, None);
        }
    }

    unsafe fn create_text_vertex_buffer(&mut self, max_vertices: usize) -> VkResult<()> {
        self.text_vb_size = (mem::size_of::<TextVertex>() * max_vertices) as vk::DeviceSize;

        let info = vk::BufferCreateInfo::default()
            .size(self.text_vb_size)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        self.text_vb = self.device.create_buffer(&info, None)?;
        let req = self.device.get_buffer_memory_requirements(self.text_vb);

        let mem_type = find_memory_type(
            &self.instance,
            self.physical_device,
            req.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(req.size)
            .memory_type_index(mem_type);

        self.text_vb_mem = self.device.allocate_memory(&alloc, None)?;
        self.device.bind_buffer_memory(self.text_vb, self.text_vb_mem, 0)?;
        Ok(())
    }

    unsafe fn create_font_resources(&mut self, atlas_r8: &[u8]) -> VkResult<()> {
        let staging_size = atlas_r8.len() as vk::DeviceSize;

        let (staging_buf, staging_mem) = create_buffer(
            &self.instance,
            self.physical_device,
            &self.device,
            staging_size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        let ptr_map = self
            .device
            .map_memory(staging_mem, 0, staging_size, vk::MemoryMapFlags::empty())?
            as *mut u8;

        ptr::copy_nonoverlapping(atlas_r8.as_ptr(), ptr_map, atlas_r8.len());
        self.device.unmap_memory(staging_mem);

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8_UNORM)
            .extent(vk::Extent3D {
                width: 128,
                height: 128,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        self.font_image = self.device.create_image(&image_info, None)?;
        let req = self.device.get_image_memory_requirements(self.font_image);

        let mem_type = find_memory_type(
            &self.instance,
            self.physical_device,
            req.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(req.size)
            .memory_type_index(mem_type);

        self.font_image_mem = self.device.allocate_memory(&alloc, None)?;
        self.device
            .bind_image_memory(self.font_image, self.font_image_mem, 0)?;

        // CRITICAL: use dedicated upload command pool (avoid UB / driver crash)
        immediate_submit(&self.device, self.upload_command_pool, self.queue, |cmd| {
            transition_image_layout(
                &self.device,
                cmd,
                self.font_image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );

            let region = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .mip_level(0)
                        .base_array_layer(0)
                        .layer_count(1),
                )
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(vk::Extent3D {
                    width: 128,
                    height: 128,
                    depth: 1,
                });

            self.device.cmd_copy_buffer_to_image(
                cmd,
                staging_buf,
                self.font_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                std::slice::from_ref(&region),
            );

            transition_image_layout(
                &self.device,
                cmd,
                self.font_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        })?;

        self.device.destroy_buffer(staging_buf, None);
        self.device.free_memory(staging_mem, None);

        self.font_image_view = self.device.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(self.font_image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::R8_UNORM)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1),
                ),
            None,
        )?;

        self.font_sampler = self.device.create_sampler(
            &vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::NEAREST)
                .min_filter(vk::Filter::NEAREST)
                .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
            None,
        )?;

        Ok(())
    }

    unsafe fn create_text_descriptor(&mut self) -> VkResult<()> {
        let binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);

        self.text_desc_set_layout = self.device.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo::default()
                .bindings(std::slice::from_ref(&binding)),
            None,
        )?;

        let pool_size = vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1);

        self.text_desc_pool = self.device.create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo::default()
                .max_sets(1)
                .pool_sizes(std::slice::from_ref(&pool_size)),
            None,
        )?;

        self.text_desc_set = self.device.allocate_descriptor_sets(
            &vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(self.text_desc_pool)
                .set_layouts(std::slice::from_ref(&self.text_desc_set_layout)),
        )?[0];

        let img = vk::DescriptorImageInfo::default()
            .sampler(self.font_sampler)
            .image_view(self.font_image_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.text_desc_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(std::slice::from_ref(&img));

        self.device.update_descriptor_sets(std::slice::from_ref(&write), &[]);
        Ok(())
    }

    pub(super) unsafe fn draw_text_overlay(
        &mut self,
        cmd: vk::CommandBuffer,
        text: &str,
    ) -> VkResult<()> {
        if text.is_empty() {
            return Ok(());
        }

        let vertices = build_text_vertices(text, self.extent);
        if vertices.is_empty() {
            return Ok(());
        }

        let bytes = (vertices.len() * mem::size_of::<TextVertex>()) as vk::DeviceSize;
        if bytes > self.text_vb_size {
            return Ok(());
        }

        let dst = self
            .device
            .map_memory(self.text_vb_mem, 0, bytes, vk::MemoryMapFlags::empty())?
            as *mut u8;

        ptr::copy_nonoverlapping(vertices.as_ptr() as *const u8, dst, bytes as usize);
        self.device.unmap_memory(self.text_vb_mem);

        self.device.cmd_bind_pipeline(
            cmd,
            vk::PipelineBindPoint::GRAPHICS,
            self.text_pipeline,
        );

        self.device.cmd_bind_descriptor_sets(
            cmd,
            vk::PipelineBindPoint::GRAPHICS,
            self.text_pipeline_layout,
            0,
            std::slice::from_ref(&self.text_desc_set),
            &[],
        );

        let vb = [self.text_vb];
        let offsets = [0u64];
        self.device.cmd_bind_vertex_buffers(cmd, 0, &vb, &offsets);

        self.device.cmd_draw(cmd, vertices.len() as u32, 1, 0, 0);
        Ok(())
    }
}

pub(super) fn build_text_vertices(text: &str, extent: vk::Extent2D) -> Vec<TextVertex> {
    let mut out = Vec::new();
    let mut x = 8.0f32;
    let mut y = 8.0f32;

    let w = extent.width as f32;
    let h = extent.height as f32;

    let color = [1.0, 1.0, 1.0, 1.0];

    for &b in text.as_bytes() {
        if b == b'\n' {
            x = 8.0;
            y += 10.0;
            continue;
        }

        let gx = (b & 0x0F) as f32;
        let gy = (b >> 4) as f32;

        let u0 = gx / 16.0;
        let v0 = gy / 16.0;
        let u1 = (gx + 1.0) / 16.0;
        let v1 = (gy + 1.0) / 16.0;

        let p0 = px_to_ndc(x, y, w, h);
        let p1 = px_to_ndc(x + 8.0, y, w, h);
        let p2 = px_to_ndc(x + 8.0, y + 8.0, w, h);
        let p3 = px_to_ndc(x, y + 8.0, w, h);

        out.push(TextVertex::new(p0, [u0, v0], color));
        out.push(TextVertex::new(p1, [u1, v0], color));
        out.push(TextVertex::new(p2, [u1, v1], color));

        out.push(TextVertex::new(p0, [u0, v0], color));
        out.push(TextVertex::new(p2, [u1, v1], color));
        out.push(TextVertex::new(p3, [u0, v1], color));

        x += 8.0;
    }

    out
}

pub(super) fn px_to_ndc(x_px: f32, y_px: f32, w: f32, h: f32) -> [f32; 2] {
    let x = (x_px / w) * 2.0 - 1.0;
    let y = 1.0 - (y_px / h) * 2.0;
    [x, y]
}

pub(super) fn build_font_atlas_128_r8() -> Vec<u8> {
    let mut atlas = vec![0u8; 128 * 128];

    for ch in 0u8..=127u8 {
        let glyph = font8x8(ch);
        let gx = (ch & 0x0F) as usize;
        let gy = (ch >> 4) as usize;

        let ox = gx * 8;
        let oy = gy * 8;

        for row in 0..8 {
            let bits = glyph[row];
            for col in 0..8 {
                let on = (bits & (1u8 << col)) != 0;
                atlas[(oy + row) * 128 + (ox + col)] = if on { 255 } else { 0 };
            }
        }
    }

    atlas
}

pub(super) fn font8x8(ch: u8) -> [u8; 8] {
    let c = ch as usize;
    let mut out = [0u8; 8];

    if c < 32 || c > 126 {
        return out;
    }

    let idx = c - 32;
    let table: &[[u8; 8]; 95] = &font8x8::FONT8X8_ASCII;
    out.copy_from_slice(&table[idx]);
    out
}