use crate::error::{VkRenderError, VkResult};

use ash::vk;
use ash::{Device, Entry};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::ffi::CString;
use std::time::Instant;

use super::state::{
    CoreContext, DebugState, FrameManager, PipelinePack, SwapchainContext, TextOverlayResources,
    UiOverlayResources, VulkanRenderer,
};
use super::types::{FrameSync, FRAMES_IN_FLIGHT};

use super::super::device::*;
use super::super::instance::*;
use super::super::pipeline::*;
use super::super::swapchain::*;

impl VulkanRenderer {
    pub unsafe fn new(
        display: RawDisplayHandle,
        window: RawWindowHandle,
        width: u32,
        height: u32,
    ) -> VkResult<Self> {
        let entry = Entry::load().map_err(|e| VkRenderError::AshWindow(e.to_string()))?;

        let app_name = CString::new("newengine").unwrap();
        let engine_name = CString::new("newengine").unwrap();

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 0, 1, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 0, 1, 0))
            .api_version(vk::API_VERSION_1_2);

        let mut extension_names = ash_window::enumerate_required_extensions(display)
            .map_err(|e| VkRenderError::AshWindow(e.to_string()))?
            .to_vec();

        if cfg!(debug_assertions) {
            extension_names.push(ash::ext::debug_utils::NAME.as_ptr());
        }

        let validation_layer = CString::new("VK_LAYER_KHRONOS_validation").unwrap();
        let enable_validation =
            cfg!(debug_assertions) && has_instance_layer(&entry, validation_layer.as_c_str());

        let mut layer_ptrs: Vec<*const i8> = Vec::new();
        if enable_validation {
            layer_ptrs.push(validation_layer.as_ptr());
        } else if cfg!(debug_assertions) {
            log::warn!("Vulkan validation layer not found; running without validation.");
        }

        let mut create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names);

        if enable_validation {
            create_info = create_info.enabled_layer_names(&layer_ptrs);
        }

        let instance = entry.create_instance(&create_info, None)?;

        let surface = ash_window::create_surface(&entry, &instance, display, window, None)
            .map_err(|e| VkRenderError::AshWindow(e.to_string()))?;

        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);

        let (physical_device, queue_family_index) =
            pick_physical_device(&instance, &surface_loader, surface)?;

        let (device, queue) = create_device(&instance, physical_device, queue_family_index)?;
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);

        let (swapchain, images, format, extent) = create_swapchain(
            &swapchain_loader,
            &surface_loader,
            surface,
            physical_device,
            width,
            height,
            queue_family_index,
            vk::SwapchainKHR::null(),
        )?;

        let image_views = create_image_views(&device, &images, format)?;
        let image_layouts = vec![vk::ImageLayout::UNDEFINED; images.len()];

        let render_pass = create_render_pass(&device, format)?;
        let (tri_pipeline_layout, tri_pipeline) = create_pipeline(&device, render_pass)?;
        let framebuffers = create_framebuffers(&device, render_pass, &image_views, extent)?;

        let command_pool = device.create_command_pool(
            &vk::CommandPoolCreateInfo::default()
                .queue_family_index(queue_family_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
            None,
        )?;

        let command_buffers = device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(images.len() as u32),
        )?;

        let upload_command_pool = device.create_command_pool(
            &vk::CommandPoolCreateInfo::default()
                .queue_family_index(queue_family_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
            None,
        )?;

        let make_frame = |device: &Device| -> VkResult<FrameSync> {
            let image_available =
                device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
            let render_finished =
                device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
            let in_flight = device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )?;
            Ok(FrameSync {
                image_available,
                render_finished,
                in_flight,
            })
        };

        let frames = [make_frame(&device)?, make_frame(&device)?];
        let images_in_flight = vec![vk::Fence::null(); images.len()];

        let core = CoreContext {
            instance,
            surface_loader,
            surface,
            physical_device,
            device,
            queue_family_index,
            queue,
            swapchain_loader,
        };

        let swapchain = SwapchainContext {
            swapchain,
            images,
            image_views,
            format,
            extent,
            framebuffers,
            image_layouts,
        };

        let pipelines = PipelinePack {
            render_pass,
            tri_pipeline_layout,
            tri_pipeline,
            text_pipeline_layout: vk::PipelineLayout::null(),
            text_pipeline: vk::Pipeline::null(),
            ui_pipeline_layout: vk::PipelineLayout::null(),
            ui_pipeline: vk::Pipeline::null(),
        };

        let text = TextOverlayResources {
            desc_set_layout: vk::DescriptorSetLayout::null(),
            desc_pool: vk::DescriptorPool::null(),
            desc_set: vk::DescriptorSet::null(),

            font_image: vk::Image::null(),
            font_image_mem: vk::DeviceMemory::null(),
            font_image_view: vk::ImageView::null(),
            font_sampler: vk::Sampler::null(),

            vb: vk::Buffer::null(),
            vb_mem: vk::DeviceMemory::null(),
            vb_size: 0,
        };

        let ui = UiOverlayResources {
            desc_set_layout: vk::DescriptorSetLayout::null(),
            desc_pool: vk::DescriptorPool::null(),
            sampler: vk::Sampler::null(),
            textures: std::collections::HashMap::new(),

            vb: vk::Buffer::null(),
            vb_mem: vk::DeviceMemory::null(),
            vb_size: 0,

            ib: vk::Buffer::null(),
            ib_mem: vk::DeviceMemory::null(),
            ib_size: 0,

            staging_buf: vk::Buffer::null(),
            staging_mem: vk::DeviceMemory::null(),
            staging_size: 0,
        };

        let debug = DebugState {
            debug_text: String::new(),
            start_time: Instant::now(),
            pending_ui: None,
            target_width: width,
            target_height: height,
        };

        let mut me = Self {
            core,
            swapchain,
            pipelines,
            frames: FrameManager {
                frames,
                frame_index: 0,
                images_in_flight,
                command_pool,
                command_buffers,
                upload_command_pool,
            },
            text,
            ui,
            debug,
        };

        me.init_text_overlay()?;
        me.init_ui_overlay()?;

        Ok(me)
    }
}
