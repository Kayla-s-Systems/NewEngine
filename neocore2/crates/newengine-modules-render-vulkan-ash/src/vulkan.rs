use crate::error::{VkRenderError, VkResult};

use ash::vk;
use ash::{Device, Entry, Instance};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::ffi::{CStr, CString};

pub struct VulkanRenderer {
    instance: Instance,

    surface_loader: ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,

    physical_device: vk::PhysicalDevice,
    device: Device,

    queue_family_index: u32,
    queue: vk::Queue,

    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    swapchain_format: vk::Format,
    extent: vk::Extent2D,

    image_layouts: Vec<vk::ImageLayout>,

    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,

    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    in_flight: vk::Fence,

    target_width: u32,
    target_height: u32,
}

impl VulkanRenderer {
    pub unsafe fn new(
        display: RawDisplayHandle,
        window: RawWindowHandle,
        width: u32,
        height: u32,
    ) -> VkResult<Self> {
        let entry = unsafe { Entry::load().map_err(|e| VkRenderError::AshWindow(e.to_string()))? };

        let app_name = CString::new("newengine").unwrap();
        let engine_name = CString::new("newengine").unwrap();

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 0, 1, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 0, 1, 0))
            .api_version(vk::API_VERSION_1_2);

        let mut extension_names =
            ash_window::enumerate_required_extensions(display)
                .map_err(|e| VkRenderError::AshWindow(e.to_string()))?
                .to_vec();

        // Debug utils is optional; keep it only for debug builds.
        if cfg!(debug_assertions) {
            extension_names.push(ash::ext::debug_utils::NAME.as_ptr());
        }

        let validation_layer = CString::new("VK_LAYER_KHRONOS_validation").unwrap();
        let enable_validation = cfg!(debug_assertions) && has_instance_layer(&entry, validation_layer.as_c_str());
        let mut layer_ptrs: Vec<*const i8> = Vec::new();

        if enable_validation {
            layer_ptrs.push(validation_layer.as_ptr());
        } else if cfg!(debug_assertions) {
            log::warn!("Vulkan validation layer not found; running without validation.");
        }

        let create_info = if enable_validation {
            vk::InstanceCreateInfo::default()
                .application_info(&app_info)
                .enabled_layer_names(&layer_ptrs)
                .enabled_extension_names(&extension_names)
        } else {
            vk::InstanceCreateInfo::default()
                .application_info(&app_info)
                .enabled_extension_names(&extension_names)
        };

        let instance = unsafe { entry.create_instance(&create_info, None)? };

        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, display, window, None)
                .map_err(|e| VkRenderError::AshWindow(e.to_string()))?
        };

        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);

        let (physical_device, queue_family_index) =
            pick_physical_device(&instance, &surface_loader, surface)?;

        let (device, queue) = create_device(&instance, physical_device, queue_family_index)?;

        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);

        let (swapchain, swapchain_images, swapchain_format, extent) = create_swapchain(
            &swapchain_loader,
            &surface_loader,
            surface,
            physical_device,
            width,
            height,
            queue_family_index,
        )?;

        let swapchain_image_views =
            create_image_views(&device, &swapchain_images, swapchain_format)?;

        let image_layouts = vec![vk::ImageLayout::UNDEFINED; swapchain_images.len()];

        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family_index)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?
        };

        let command_buffers = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(swapchain_images.len() as u32),
            )?
        };

        let image_available =
            unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };
        let render_finished =
            unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };
        let in_flight = unsafe {
            device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )?
        };

        Ok(Self {
            instance,
            surface_loader,
            surface,

            physical_device,
            device,

            queue_family_index,
            queue,

            swapchain_loader,
            swapchain,
            swapchain_images,
            swapchain_image_views,
            swapchain_format,
            extent,

            image_layouts,

            command_pool,
            command_buffers,

            image_available,
            render_finished,
            in_flight,

            target_width: width,
            target_height: height,
        })
    }

    #[inline]
    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.target_width = width;
        self.target_height = height;
    }

    pub fn draw_clear(&mut self) -> VkResult<()> {
        // Minimized window -> skip
        if self.target_width == 0 || self.target_height == 0 {
            return Ok(());
        }

        unsafe {
            self.device
                .wait_for_fences(&[self.in_flight], true, u64::MAX)?;
            self.device.reset_fences(&[self.in_flight])?;
        }

        let (image_index, _suboptimal) = match unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            )
        } {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain()?;
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };

        let idx = image_index as usize;
        let cmd = self.command_buffers[idx];
        let image = self.swapchain_images[idx];

        unsafe {
            self.device
                .reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;

            self.device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;

            let old_layout = self.image_layouts[idx];
            transition_image(
                &self.device,
                cmd,
                image,
                old_layout,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );

            // Fixed color => no flicker
            let clear_color = vk::ClearColorValue {
                float32: [0.10, 0.12, 0.16, 1.0],
            };

            let ranges = [vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            }];

            self.device.cmd_clear_color_image(
                cmd,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &clear_color,
                &ranges,
            );

            transition_image(
                &self.device,
                cmd,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );

            self.image_layouts[idx] = vk::ImageLayout::PRESENT_SRC_KHR;

            self.device.end_command_buffer(cmd)?;

            let wait_stages = [vk::PipelineStageFlags::TRANSFER];
            let wait_sems = [self.image_available];
            let signal_sems = [self.render_finished];
            let cmd_bufs = [cmd];

            let submit_infos = [vk::SubmitInfo::default()
                .wait_semaphores(&wait_sems)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&cmd_bufs)
                .signal_semaphores(&signal_sems)];

            self.device
                .queue_submit(self.queue, &submit_infos, self.in_flight)?;

            let swapchains = [self.swapchain];
            let indices = [image_index];

            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal_sems)
                .swapchains(&swapchains)
                .image_indices(&indices);

            match self.swapchain_loader.queue_present(self.queue, &present_info) {
                Ok(_) => {}
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                    self.recreate_swapchain()?;
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    fn recreate_swapchain(&mut self) -> VkResult<()> {
        if self.target_width == 0 || self.target_height == 0 {
            return Ok(());
        }

        unsafe {
            let _ = self.device.device_wait_idle();

            for &iv in &self.swapchain_image_views {
                self.device.destroy_image_view(iv, None);
            }
            self.swapchain_image_views.clear();

            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
        }

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

        // Command buffers count must match images count
        unsafe {
            self.device
                .free_command_buffers(self.command_pool, &self.command_buffers);

            self.command_buffers = self.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(swapchain_images.len() as u32),
            )?;
        }

        self.swapchain = swapchain;
        self.swapchain_images = swapchain_images;
        self.swapchain_format = swapchain_format;
        self.extent = extent;
        self.swapchain_image_views = swapchain_image_views;
        self.image_layouts = vec![vk::ImageLayout::UNDEFINED; self.swapchain_images.len()];

        Ok(())
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();

            self.device.destroy_fence(self.in_flight, None);
            self.device.destroy_semaphore(self.render_finished, None);
            self.device.destroy_semaphore(self.image_available, None);

            self.device
                .free_command_buffers(self.command_pool, &self.command_buffers);
            self.device.destroy_command_pool(self.command_pool, None);

            for &iv in &self.swapchain_image_views {
                self.device.destroy_image_view(iv, None);
            }

            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            self.surface_loader.destroy_surface(self.surface, None);

            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

unsafe fn has_instance_layer(entry: &Entry, name: &CStr) -> bool {
    let Ok(props) = entry.enumerate_instance_layer_properties() else {
        return false;
    };

    props.iter().any(|p| {
        let layer = unsafe { CStr::from_ptr(p.layer_name.as_ptr()) };
        layer == name
    })
}

fn pick_physical_device(
    instance: &Instance,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> VkResult<(vk::PhysicalDevice, u32)> {
    let pds = unsafe { instance.enumerate_physical_devices()? };
    for pd in pds {
        let qf = unsafe { instance.get_physical_device_queue_family_properties(pd) };
        for (i, props) in qf.iter().enumerate() {
            if !props.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                continue;
            }
            let present = unsafe {
                surface_loader.get_physical_device_surface_support(pd, i as u32, surface)?
            };
            if present {
                return Ok((pd, i as u32));
            }
        }
    }
    Err(VkRenderError::AshWindow(
        "No suitable physical device found".to_string(),
    ))
}

fn create_device(
    instance: &Instance,
    pd: vk::PhysicalDevice,
    qfi: u32,
) -> VkResult<(Device, vk::Queue)> {
    let priorities = [1.0f32];
    let qci = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(qfi)
        .queue_priorities(&priorities)];

    let device_exts = [ash::khr::swapchain::NAME.as_ptr()];

    let dci = vk::DeviceCreateInfo::default()
        .queue_create_infos(&qci)
        .enabled_extension_names(&device_exts);

    let device = unsafe { instance.create_device(pd, &dci, None)? };
    let queue = unsafe { device.get_device_queue(qfi, 0) };
    Ok((device, queue))
}

fn create_swapchain(
    swapchain_loader: &ash::khr::swapchain::Device,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    pd: vk::PhysicalDevice,
    width: u32,
    height: u32,
    qfi: u32,
) -> VkResult<(vk::SwapchainKHR, Vec<vk::Image>, vk::Format, vk::Extent2D)> {
    let caps = unsafe { surface_loader.get_physical_device_surface_capabilities(pd, surface)? };
    let formats = unsafe { surface_loader.get_physical_device_surface_formats(pd, surface)? };
    let present_modes =
        unsafe { surface_loader.get_physical_device_surface_present_modes(pd, surface)? };

    let format = formats
        .iter()
        .find(|f| f.format == vk::Format::B8G8R8A8_UNORM)
        .cloned()
        .unwrap_or(formats[0]);

    let present_mode = present_modes
        .into_iter()
        .find(|&m| m == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO);

    let extent = if caps.current_extent.width != u32::MAX {
        caps.current_extent
    } else {
        vk::Extent2D {
            width: width
                .clamp(caps.min_image_extent.width, caps.max_image_extent.width)
                .max(1),
            height: height
                .clamp(caps.min_image_extent.height, caps.max_image_extent.height)
                .max(1),
        }
    };

    let mut image_count = caps.min_image_count + 1;
    if caps.max_image_count != 0 {
        image_count = image_count.min(caps.max_image_count);
    }

    let queue_family_indices = [qfi];

    let sci = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(image_count)
        .image_format(format.format)
        .image_color_space(format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::TRANSFER_DST)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(caps.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .queue_family_indices(&queue_family_indices);

    let swapchain = unsafe { swapchain_loader.create_swapchain(&sci, None)? };
    let images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };

    Ok((swapchain, images, format.format, extent))
}

fn create_image_views(
    device: &Device,
    images: &[vk::Image],
    format: vk::Format,
) -> VkResult<Vec<vk::ImageView>> {
    let mut views = Vec::with_capacity(images.len());
    for &img in images {
        let iv = unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(img)
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

fn transition_image(
    device: &Device,
    cmd: vk::CommandBuffer,
    img: vk::Image,
    old: vk::ImageLayout,
    new: vk::ImageLayout,
) {
    let (src_stage, src_access) = match old {
        vk::ImageLayout::UNDEFINED => (vk::PipelineStageFlags::TOP_OF_PIPE, vk::AccessFlags::empty()),
        vk::ImageLayout::PRESENT_SRC_KHR => (vk::PipelineStageFlags::BOTTOM_OF_PIPE, vk::AccessFlags::empty()),
        vk::ImageLayout::TRANSFER_DST_OPTIMAL => (
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_WRITE,
        ),
        _ => (vk::PipelineStageFlags::ALL_COMMANDS, vk::AccessFlags::MEMORY_WRITE),
    };

    let (dst_stage, dst_access) = match new {
        vk::ImageLayout::TRANSFER_DST_OPTIMAL => (
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_WRITE,
        ),
        vk::ImageLayout::PRESENT_SRC_KHR => (vk::PipelineStageFlags::BOTTOM_OF_PIPE, vk::AccessFlags::empty()),
        _ => (vk::PipelineStageFlags::ALL_COMMANDS, vk::AccessFlags::MEMORY_READ),
    };

    let barriers = [vk::ImageMemoryBarrier::default()
        .old_layout(old)
        .new_layout(new)
        .src_access_mask(src_access)
        .dst_access_mask(dst_access)
        .image(img)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        )];

    unsafe {
        device.cmd_pipeline_barrier(
            cmd,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &barriers,
        );
    }
}