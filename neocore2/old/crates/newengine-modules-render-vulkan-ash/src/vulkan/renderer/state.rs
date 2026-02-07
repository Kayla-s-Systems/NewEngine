use ash::vk;
use newengine_ui::draw::UiDrawList;
use std::collections::HashMap;
use std::time::Instant;

use super::types::{FrameSync, FRAMES_IN_FLIGHT};
use crate::vulkan::ui::GpuUiTexture;

pub struct CoreContext {
    pub(crate) instance: ash::Instance,

    pub(crate) surface_loader: ash::khr::surface::Instance,
    pub(crate) surface: vk::SurfaceKHR,

    pub(crate) physical_device: vk::PhysicalDevice,
    pub(crate) device: ash::Device,

    pub(crate) queue_family_index: u32,
    pub(crate) queue: vk::Queue,

    pub(crate) swapchain_loader: ash::khr::swapchain::Device,
}

pub struct SwapchainContext {
    pub(crate) swapchain: vk::SwapchainKHR,
    pub(crate) images: Vec<vk::Image>,
    pub(crate) image_views: Vec<vk::ImageView>,
    pub(crate) format: vk::Format,
    pub(crate) extent: vk::Extent2D,
    pub(crate) framebuffers: Vec<vk::Framebuffer>,
    pub(crate) image_layouts: Vec<vk::ImageLayout>,
}

pub struct PipelinePack {
    pub(crate) render_pass: vk::RenderPass,

    pub(crate) tri_pipeline_layout: vk::PipelineLayout,
    pub(crate) tri_pipeline: vk::Pipeline,

    pub(crate) text_pipeline_layout: vk::PipelineLayout,
    pub(crate) text_pipeline: vk::Pipeline,

    pub(crate) ui_pipeline_layout: vk::PipelineLayout,
    pub(crate) ui_pipeline: vk::Pipeline,
}

pub struct FrameManager {
    pub(crate) frames: [FrameSync; FRAMES_IN_FLIGHT],
    pub(crate) frame_index: usize,
    pub(crate) images_in_flight: Vec<vk::Fence>,
    pub(crate) command_pool: vk::CommandPool,
    pub(crate) command_buffers: Vec<vk::CommandBuffer>,
    pub(crate) upload_command_pool: vk::CommandPool,
}

pub struct TextOverlayResources {
    pub(crate) desc_set_layout: vk::DescriptorSetLayout,
    pub(crate) desc_pool: vk::DescriptorPool,
    pub(crate) desc_set: vk::DescriptorSet,

    pub(crate) font_image: vk::Image,
    pub(crate) font_image_mem: vk::DeviceMemory,
    pub(crate) font_image_view: vk::ImageView,
    pub(crate) font_sampler: vk::Sampler,

    pub(crate) vb: vk::Buffer,
    pub(crate) vb_mem: vk::DeviceMemory,
    pub(crate) vb_size: vk::DeviceSize,
}

pub struct UiOverlayResources {
    pub(crate) desc_set_layout: vk::DescriptorSetLayout,
    pub(crate) desc_pool: vk::DescriptorPool,
    pub(crate) sampler: vk::Sampler,

    pub(crate) textures: HashMap<u32, GpuUiTexture>,

    pub(crate) vb: vk::Buffer,
    pub(crate) vb_mem: vk::DeviceMemory,
    pub(crate) vb_size: vk::DeviceSize,

    pub(crate) ib: vk::Buffer,
    pub(crate) ib_mem: vk::DeviceMemory,
    pub(crate) ib_size: vk::DeviceSize,

    pub(crate) staging_buf: vk::Buffer,
    pub(crate) staging_mem: vk::DeviceMemory,
    pub(crate) staging_size: vk::DeviceSize,
}

pub struct DebugState {
    pub(crate) debug_text: String,
    pub(crate) start_time: Instant,

    pub(crate) pending_ui: Option<UiDrawList>,

    pub(crate) target_width: u32,
    pub(crate) target_height: u32,

    // Deferred swapchain resize.
    pub(crate) swapchain_dirty: bool,

    // Per-frame recording state.
    pub(crate) in_frame: bool,
    pub(crate) current_image_index: u32,
    pub(crate) current_swapchain_idx: usize,
}

pub struct VulkanRenderer {
    pub(crate) core: CoreContext,
    pub(crate) swapchain: SwapchainContext,
    pub(crate) pipelines: PipelinePack,
    pub(crate) frames: FrameManager,
    pub(crate) text: TextOverlayResources,
    pub(crate) ui: UiOverlayResources,
    pub(crate) debug: DebugState,
}
