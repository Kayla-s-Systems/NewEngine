use crate::error::VkResult;

use ash::vk;
use std::mem;

use super::super::pipeline::create_shader_module;

#[repr(C)]
#[derive(Clone, Copy)]
struct UiPc {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

pub unsafe fn create_ui_pipeline(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
) -> VkResult<(vk::PipelineLayout, vk::Pipeline)> {
    let vert = create_shader_module(
        device,
        include_bytes!(concat!(env!("OUT_DIR"), "/ui.vert.spv")),
    )?;
    let frag = create_shader_module(
        device,
        include_bytes!(concat!(env!("OUT_DIR"), "/ui.frag.spv")),
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
        .stride(mem::size_of::<newengine_ui::draw::UiVertex>() as u32)
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
            .format(vk::Format::R8G8B8A8_UNORM)
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

    let cb =
        vk::PipelineColorBlendStateCreateInfo::default().attachments(std::slice::from_ref(&ca));

    let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let ds = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);

    let push_ranges = [vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size(mem::size_of::<UiPc>() as u32)];

    let set_layouts = [set_layout];
    let layout = device.create_pipeline_layout(
        &vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(&push_ranges),
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

pub(super) fn ui_pc_bytes(screen_size_px: [u32; 2]) -> [u8; std::mem::size_of::<UiPc>()] {
    let pc = UiPc {
        screen_size: [screen_size_px[0] as f32, screen_size_px[1] as f32],
        _pad: [0.0, 0.0],
    };

    unsafe { std::mem::transmute::<UiPc, [u8; std::mem::size_of::<UiPc>()]>(pc) }
}
