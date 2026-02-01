use crate::error::{VkRenderError, VkResult};

use ash::vk;
use ash::{Device, Entry, Instance};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::ffi::CString;

use super::device::*;
use super::instance::*;
use super::pipeline::*;
use super::swapchain::*;
use super::util::*;

pub struct VulkanRenderer {
    pub(super) instance: Instance,

    pub(super) render_pass: vk::RenderPass,
    pub(super) debug_text: String,
    pub(super) framebuffers: Vec<vk::Framebuffer>,

    pub(super) pipeline_layout: vk::PipelineLayout,
    pub(super) pipeline: vk::Pipeline,

    pub(super) surface_loader: ash::khr::surface::Instance,
    pub(super) surface: vk::SurfaceKHR,

    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) device: Device,

    pub(super) queue_family_index: u32,
    pub(super) queue: vk::Queue,

    pub(super) swapchain_loader: ash::khr::swapchain::Device,
    pub(super) swapchain: vk::SwapchainKHR,
    pub(super) swapchain_images: Vec<vk::Image>,
    pub(super) swapchain_image_views: Vec<vk::ImageView>,
    pub(super) swapchain_format: vk::Format,
    pub(super) extent: vk::Extent2D,


    pub(super) upload_command_pool: vk::CommandPool,
    pub(super) image_layouts: Vec<vk::ImageLayout>,

    pub(super) command_pool: vk::CommandPool,
    pub(super) command_buffers: Vec<vk::CommandBuffer>,

    pub(super) image_available: vk::Semaphore,
    pub(super) render_finished: vk::Semaphore,
    pub(super) in_flight: vk::Fence,

    pub(super) target_width: u32,
    pub(super) target_height: u32,

    pub(super) text_pipeline_layout: vk::PipelineLayout,
    pub(super) text_pipeline: vk::Pipeline,

    pub(super) text_desc_set_layout: vk::DescriptorSetLayout,
    pub(super) text_desc_pool: vk::DescriptorPool,
    pub(super) text_desc_set: vk::DescriptorSet,

    pub(super) font_image: vk::Image,
    pub(super) font_image_mem: vk::DeviceMemory,
    pub(super) font_image_view: vk::ImageView,
    pub(super) font_sampler: vk::Sampler,

    pub(super) text_vb: vk::Buffer,
    pub(super) text_vb_mem: vk::DeviceMemory,
    pub(super) text_vb_size: vk::DeviceSize,
}

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

        let render_pass = create_render_pass(&device, swapchain_format)?;
        let (pipeline_layout, pipeline) = create_pipeline(&device, render_pass)?;
        let framebuffers =
            create_framebuffers(&device, render_pass, &swapchain_image_views, extent)?;

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
                .command_buffer_count(swapchain_images.len() as u32),
        )?;
        let upload_command_pool = device.create_command_pool(
            &vk::CommandPoolCreateInfo::default()
                .queue_family_index(queue_family_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
            None,
        )?;


        let image_available = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
        let render_finished = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
        let in_flight = device.create_fence(
            &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
            None,
        )?;

        let mut me = Self {
            instance,

            debug_text: String::new(),
            render_pass,
            framebuffers,

            pipeline_layout,
            pipeline,

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

            upload_command_pool,
            image_layouts,

            command_pool,
            command_buffers,

            image_available,
            render_finished,
            in_flight,

            target_width: width,
            target_height: height,

            text_pipeline_layout: vk::PipelineLayout::null(),
            text_pipeline: vk::Pipeline::null(),

            text_desc_set_layout: vk::DescriptorSetLayout::null(),
            text_desc_pool: vk::DescriptorPool::null(),
            text_desc_set: vk::DescriptorSet::null(),

            font_image: vk::Image::null(),
            font_image_mem: vk::DeviceMemory::null(),
            font_image_view: vk::ImageView::null(),
            font_sampler: vk::Sampler::null(),

            text_vb: vk::Buffer::null(),
            text_vb_mem: vk::DeviceMemory::null(),
            text_vb_size: 0,
        };

        me.init_text_overlay()?;
        Ok(me)
    }

    #[inline]
    pub fn set_debug_text(&mut self, text: &str) {
        self.debug_text.clear();
        self.debug_text.push_str(text);
    }

    pub fn resize(&mut self, width: u32, height: u32) -> VkResult<()> {
        self.set_target_size(width, height);
        unsafe { self.recreate_swapchain() }
    }

    #[inline]
    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.target_width = width;
        self.target_height = height;
    }

    #[inline]
    pub fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    #[inline]
    pub fn format(&self) -> vk::Format {
        self.swapchain_format
    }

    #[inline]
    pub fn draw_clear(&mut self) -> VkResult<()> {
        self.draw_clear_color([0.10, 0.12, 0.16, 1.0])
    }

    pub fn draw_clear_color(&mut self, rgba: [f32; 4]) -> VkResult<()> {
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
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => unsafe {
                self.recreate_swapchain()?;
                return Ok(());
            },
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
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );

            let clear = vk::ClearValue {
                color: vk::ClearColorValue { float32: rgba },
            };

            let rp_begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(self.framebuffers[idx])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.extent,
                })
                .clear_values(std::slice::from_ref(&clear));

            self.device
                .cmd_begin_render_pass(cmd, &rp_begin, vk::SubpassContents::INLINE);

            self.device
                .cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: self.extent.width as f32,
                height: self.extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.extent,
            };

            self.device.cmd_set_viewport(cmd, 0, std::slice::from_ref(&viewport));
            self.device.cmd_set_scissor(cmd, 0, std::slice::from_ref(&scissor));

            self.device.cmd_draw(cmd, 3, 1, 0, 0);
            let dbg = self.debug_text.clone();
            self.draw_text_overlay(cmd, &dbg)?;



            self.device.cmd_end_render_pass(cmd);

            transition_image(
                &self.device,
                cmd,
                image,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );

            self.image_layouts[idx] = vk::ImageLayout::PRESENT_SRC_KHR;

            self.device.end_command_buffer(cmd)?;

            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
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

            match self
                .swapchain_loader
                .queue_present(self.queue, &present_info)
            {
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
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();

            self.destroy_text_overlay();
            self.device.destroy_command_pool(self.upload_command_pool, None);


            self.device.destroy_fence(self.in_flight, None);
            self.device.destroy_semaphore(self.render_finished, None);
            self.device.destroy_semaphore(self.image_available, None);

            self.device
                .free_command_buffers(self.command_pool, &self.command_buffers);
            self.device.destroy_command_pool(self.command_pool, None);

            for &fb in &self.framebuffers {
                self.device.destroy_framebuffer(fb, None);
            }
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);

            for &iv in &self.swapchain_image_views {
                self.device.destroy_image_view(iv, None);
            }

            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.surface_loader.destroy_surface(self.surface, None);

            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}