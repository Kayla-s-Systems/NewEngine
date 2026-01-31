use crate::error::{VkRenderError, VkResult};

use ash::vk;
use ash::{Device, Entry, Instance};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::ffi::{CStr, CString};

pub struct VulkanRenderer {
    instance: Instance,

    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,

    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,

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
        let framebuffers = create_framebuffers(&device, render_pass, &swapchain_image_views, extent)?;

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

        let image_available = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
        let render_finished = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
        let in_flight = device.create_fence(
            &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
            None,
        )?;

        Ok(Self {
            instance,

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

    #[allow(dead_code)]
    #[inline]
    pub fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    #[allow(dead_code)]
    #[inline]
    pub fn format(&self) -> vk::Format {
        self.swapchain_format
    }

    #[allow(dead_code)]
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

    unsafe fn recreate_swapchain(&mut self) -> VkResult<()> {
        if self.target_width == 0 || self.target_height == 0 {
            return Ok(());
        }

        unsafe {
            let _ = self.device.device_wait_idle();
        }

        unsafe {
            for &fb in &self.framebuffers {
                self.device.destroy_framebuffer(fb, None);
            }
            self.framebuffers.clear();

            for &iv in &self.swapchain_image_views {
                self.device.destroy_image_view(iv, None);
            }
            self.swapchain_image_views.clear();

            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
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

        let format_changed = swapchain_format != self.swapchain_format;

        if format_changed {
            unsafe {
                self.device.destroy_pipeline(self.pipeline, None);
                self.device.destroy_pipeline_layout(self.pipeline_layout, None);
                self.device.destroy_render_pass(self.render_pass, None);
            }

            self.swapchain_format = swapchain_format;
            self.render_pass = create_render_pass(&self.device, self.swapchain_format)?;
            let (pl, p) = create_pipeline(&self.device, self.render_pass)?;
            self.pipeline_layout = pl;
            self.pipeline = p;
        } else {
            self.swapchain_format = swapchain_format;
        }

        let framebuffers =
            create_framebuffers(&self.device, self.render_pass, &swapchain_image_views, extent)?;

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
        self.extent = extent;
        self.swapchain_image_views = swapchain_image_views;
        self.framebuffers = framebuffers;
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

unsafe fn has_instance_layer(entry: &Entry, name: &CStr) -> bool {
    let Ok(props) = entry.enumerate_instance_layer_properties() else {
        return false;
    };

    props.iter().any(|p| {
        let layer = CStr::from_ptr(p.layer_name.as_ptr());
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

    let surface_format = formats
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

    let image_usage = vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::COLOR_ATTACHMENT;

    let sci = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(image_usage)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(caps.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .queue_family_indices(&queue_family_indices);

    let swapchain = unsafe { swapchain_loader.create_swapchain(&sci, None)? };
    let images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };

    Ok((swapchain, images, surface_format.format, extent))
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

unsafe fn create_render_pass(device: &Device, format: vk::Format) -> VkResult<vk::RenderPass> {
    let color_attach = vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

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
        .attachments(std::slice::from_ref(&color_attach))
        .subpasses(std::slice::from_ref(&subpass))
        .dependencies(std::slice::from_ref(&dep));

    Ok(device.create_render_pass(&rp, None)?)
}

unsafe fn create_framebuffers(
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

unsafe fn create_shader_module(device: &Device, bytes: &[u8]) -> VkResult<vk::ShaderModule> {
    let words = ash::util::read_spv(&mut std::io::Cursor::new(bytes))
        .map_err(|e| VkRenderError::AshWindow(e.to_string()))?;
    let ci = vk::ShaderModuleCreateInfo::default().code(&words);
    Ok(device.create_shader_module(&ci, None)?)
}

unsafe fn create_pipeline(
    device: &Device,
    render_pass: vk::RenderPass,
) -> VkResult<(vk::PipelineLayout, vk::Pipeline)> {
    let vert = create_shader_module(
        device,
        include_bytes!(concat!(env!("OUT_DIR"), "/tri.vert.spv")),
    )?;
    let frag = create_shader_module(
        device,
        include_bytes!(concat!(env!("OUT_DIR"), "/tri.frag.spv")),
    )?;

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

    let layout = device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default(), None)?;

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
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),
        vk::ImageLayout::PRESENT_SRC_KHR => (
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::AccessFlags::MEMORY_READ,
        ),
        _ => (
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
        ),
    }
}

fn transition_image(
    device: &Device,
    cmd: vk::CommandBuffer,
    img: vk::Image,
    old: vk::ImageLayout,
    new: vk::ImageLayout,
) {
    if old == new {
        return;
    }

    let (src_stage, src_access) = stage_access_for_layout(old);
    let (dst_stage, dst_access) = stage_access_for_layout(new);

    let barrier = vk::ImageMemoryBarrier::default()
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
        );

    unsafe {
        device.cmd_pipeline_barrier(
            cmd,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
    }
}