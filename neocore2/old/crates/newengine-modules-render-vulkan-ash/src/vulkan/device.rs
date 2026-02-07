use crate::error::{VkRenderError, VkResult};

use ash::vk;
use ash::{Device, Instance};
use std::ffi::CStr;

#[inline]
fn has_device_extension(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    required: &CStr,
) -> bool {
    let props = unsafe {
        instance
            .enumerate_device_extension_properties(physical_device)
            .unwrap_or_default()
    };

    props.iter().any(|p| unsafe {
        CStr::from_ptr(p.extension_name.as_ptr()) == required
    })
}

pub(super) fn pick_physical_device(
    instance: &Instance,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> VkResult<(vk::PhysicalDevice, u32)> {
    let devices = unsafe { instance.enumerate_physical_devices()? };
    if devices.is_empty() {
        return Err(VkRenderError::AshWindow(
            "No Vulkan physical devices found".into(),
        ));
    }

    let req_ext = ash::khr::swapchain::NAME;

    for &pd in &devices {
        // Must support swapchain extension, иначе UB при создании swapchain.
        if !has_device_extension(instance, pd, req_ext) {
            continue;
        }

        // Must have surface formats / present modes.
        let formats = unsafe {
            surface_loader.get_physical_device_surface_formats(pd, surface)
        }?;
        if formats.is_empty() {
            continue;
        }

        let present_modes = unsafe {
            surface_loader.get_physical_device_surface_present_modes(pd, surface)
        }?;
        if present_modes.is_empty() {
            continue;
        }

        // Find queue family supporting graphics + present.
        let qprops = unsafe { instance.get_physical_device_queue_family_properties(pd) };
        for (i, q) in qprops.iter().enumerate() {
            if !q.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                continue;
            }

            let supports_present = unsafe {
                surface_loader.get_physical_device_surface_support(pd, i as u32, surface)
            }?;

            if supports_present {
                return Ok((pd, i as u32));
            }
        }
    }

    Err(VkRenderError::AshWindow(
        "No suitable Vulkan physical device found (needs graphics+present queue and VK_KHR_swapchain)".into(),
    ))
}

pub(super) fn create_device(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
) -> VkResult<(Device, vk::Queue)> {
    let queue_priorities = [1.0f32];

    let queue_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&queue_priorities);

    // Enable required device extensions.
    let device_extensions = [ash::khr::swapchain::NAME.as_ptr()];

    let device_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&queue_info))
        .enabled_extension_names(&device_extensions);

    let device = unsafe { instance.create_device(physical_device, &device_info, None)? };
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

    Ok((device, queue))
}

pub(super) fn find_memory_type(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    type_bits: u32,
    props: vk::MemoryPropertyFlags,
) -> VkResult<u32> {
    let mem = unsafe { instance.get_physical_device_memory_properties(physical_device) };

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

pub(super) fn create_buffer(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    device: &Device,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    props: vk::MemoryPropertyFlags,
) -> VkResult<(vk::Buffer, vk::DeviceMemory)> {
    let info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let buffer = unsafe { device.create_buffer(&info, None)? };
    let req = unsafe { device.get_buffer_memory_requirements(buffer) };

    let mem_type = find_memory_type(instance, physical_device, req.memory_type_bits, props)?;

    let alloc = vk::MemoryAllocateInfo::default()
        .allocation_size(req.size)
        .memory_type_index(mem_type);

    let memory = unsafe { device.allocate_memory(&alloc, None)? };
    unsafe { device.bind_buffer_memory(buffer, memory, 0)? };

    Ok((buffer, memory))
}