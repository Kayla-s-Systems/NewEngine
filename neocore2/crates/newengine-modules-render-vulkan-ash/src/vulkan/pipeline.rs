use crate::error::{VkRenderError, VkResult};

use ash::vk;
use ash::Device;
use std::ffi::CString;

pub(super) unsafe fn create_render_pass(device: &Device, format: vk::Format) -> VkResult<vk::RenderPass> {
    let color = vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

    let color_ref = vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(std::slice::from_ref(&color_ref));

    let dep = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

    let rp = vk::RenderPassCreateInfo::default()
        .attachments(std::slice::from_ref(&color))
        .subpasses(std::slice::from_ref(&subpass))
        .dependencies(std::slice::from_ref(&dep));

    Ok(device.create_render_pass(&rp, None)?)
}

pub(super) unsafe fn create_render_pass_with_depth(
    device: &Device,
    color_format: vk::Format,
    depth_format: vk::Format,
) -> VkResult<vk::RenderPass> {
    let color = vk::AttachmentDescription::default()
        .format(color_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

    let depth = vk::AttachmentDescription::default()
        .format(depth_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::DONT_CARE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

    let color_ref = vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

    let depth_ref = vk::AttachmentReference::default()
        .attachment(1)
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(std::slice::from_ref(&color_ref))
        .depth_stencil_attachment(&depth_ref);

    let dep = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE);

    let attachments = [color, depth];
    let rp = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(std::slice::from_ref(&subpass))
        .dependencies(std::slice::from_ref(&dep));

    Ok(device.create_render_pass(&rp, None)?)
}

pub(super) unsafe fn create_framebuffers(
    device: &Device,
    render_pass: vk::RenderPass,
    views: &[vk::ImageView],
    extent: vk::Extent2D,
) -> VkResult<Vec<vk::Framebuffer>> {
    let mut fbs = Vec::with_capacity(views.len());
    for &view in views {
        let attachments = [view];
        let fb_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(&attachments)
            .width(extent.width)
            .height(extent.height)
            .layers(1);

        fbs.push(device.create_framebuffer(&fb_info, None)?);
    }
    Ok(fbs)
}

pub(crate) unsafe fn create_shader_module(device: &Device, bytes: &[u8]) -> VkResult<vk::ShaderModule> {
    let words = ash::util::read_spv(&mut std::io::Cursor::new(bytes))
        .map_err(|e| VkRenderError::AshWindow(e.to_string()))?;
    let ci = vk::ShaderModuleCreateInfo::default().code(&words);
    Ok(device.create_shader_module(&ci, None)?)
}

pub(crate) unsafe fn create_shader_module_words(device: &Device, words: &[u32]) -> VkResult<vk::ShaderModule> {
    let ci = vk::ShaderModuleCreateInfo::default().code(words);
    Ok(device.create_shader_module(&ci, None)?)
}

pub(super) unsafe fn create_pipeline(
    device: &Device,
    render_pass: vk::RenderPass,
    shaders: &crate::vulkan::ShaderPack,
) -> VkResult<(vk::PipelineLayout, vk::Pipeline)> {
    let vert = create_shader_module_words(device, &shaders.tri_vert)?;
    let frag = create_shader_module_words(device, &shaders.tri_frag)?;

    let entry = CString::new("main").unwrap();

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

    // Push constants: vec4 (time, aspect, 0, 0)
    let push_ranges = [vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        .offset(0)
        .size(16)];

    let layout = device.create_pipeline_layout(
        &vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&[])
            .push_constant_ranges(&push_ranges),
        None,
    )?;

    let vi = vk::PipelineVertexInputStateCreateInfo::default();

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
        .blend_enable(false)
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