use crate::error::VkResult;

use ash::vk;

/// Picks conservative stage+access masks for a given layout.
/// This is not exhaustive, but covers our engine's usage.
#[inline]
fn stage_access_for_layout(layout: vk::ImageLayout) -> (vk::PipelineStageFlags, vk::AccessFlags) {
    match layout {
        vk::ImageLayout::UNDEFINED => (
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::AccessFlags::empty(),
        ),

        vk::ImageLayout::TRANSFER_DST_OPTIMAL => (
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_WRITE,
        ),
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL => (
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_READ,
        ),

        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),

        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => (
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::AccessFlags::SHADER_READ,
        ),

        vk::ImageLayout::PRESENT_SRC_KHR => (
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::AccessFlags::empty(),
        ),

        _ => (
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
        ),
    }
}

/// Generic barrier helper. Critical: queue family indices MUST be IGNORED unless ownership transfer is intended.
#[inline]
pub unsafe fn transition_image_layout(
    device: &ash::Device,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    if old_layout == new_layout {
        return;
    }

    let (src_stage, src_access) = stage_access_for_layout(old_layout);
    let (dst_stage, dst_access) = stage_access_for_layout(new_layout);

    let barrier = vk::ImageMemoryBarrier::default()
        .src_access_mask(src_access)
        .dst_access_mask(dst_access)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        );

    device.cmd_pipeline_barrier(
        cmd,
        src_stage,
        dst_stage,
        vk::DependencyFlags::empty(),
        &[],
        &[],
        std::slice::from_ref(&barrier),
    );
}

/// Alias used by the swapchain path (kept for readability).
#[inline]
pub unsafe fn transition_image(
    device: &ash::Device,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    transition_image_layout(device, cmd, image, old_layout, new_layout);
}

/// One-shot submit utility for short copy/transition work.
/// Uses queue_wait_idle for simplicity/stability.
pub unsafe fn immediate_submit<F: FnOnce(vk::CommandBuffer)>(
    device: &ash::Device,
    pool: vk::CommandPool,
    queue: vk::Queue,
    f: F,
) -> VkResult<()> {
    let alloc = vk::CommandBufferAllocateInfo::default()
        .command_pool(pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);

    let cmd = device.allocate_command_buffers(&alloc)?[0];

    device.begin_command_buffer(
        cmd,
        &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
    )?;

    f(cmd);

    device.end_command_buffer(cmd)?;

    let submit = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd));
    device.queue_submit(queue, std::slice::from_ref(&submit), vk::Fence::null())?;
    device.queue_wait_idle(queue)?;

    device.free_command_buffers(pool, std::slice::from_ref(&cmd));
    Ok(())
}
