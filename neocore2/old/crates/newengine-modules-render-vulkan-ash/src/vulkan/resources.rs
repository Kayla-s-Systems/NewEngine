// src/vulkan/resources.rs
#![allow(dead_code)]

use crate::error::VkResult;
use ash::vk;

/// Buffer + device memory bundle.
#[derive(Clone, Copy, Default)]
pub struct BufferAlloc {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: vk::DeviceSize,
}

impl BufferAlloc {
    #[inline]
    pub const fn is_null(&self) -> bool {
        self.buffer == vk::Buffer::null()
    }

    #[inline]
    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        if self.buffer != vk::Buffer::null() {
            device.destroy_buffer(self.buffer, None);
            self.buffer = vk::Buffer::null();
        }
        if self.memory != vk::DeviceMemory::null() {
            device.free_memory(self.memory, None);
            self.memory = vk::DeviceMemory::null();
        }
        self.size = 0;
    }
}

/// Image + device memory + view + optional sampler bundle.
#[derive(Clone, Copy, Default)]
pub struct ImageAlloc {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
}

impl ImageAlloc {
    #[inline]
    pub const fn is_null(&self) -> bool {
        self.image == vk::Image::null()
    }

    #[inline]
    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        if self.sampler != vk::Sampler::null() {
            device.destroy_sampler(self.sampler, None);
            self.sampler = vk::Sampler::null();
        }
        if self.view != vk::ImageView::null() {
            device.destroy_image_view(self.view, None);
            self.view = vk::ImageView::null();
        }
        if self.image != vk::Image::null() {
            device.destroy_image(self.image, None);
            self.image = vk::Image::null();
        }
        if self.memory != vk::DeviceMemory::null() {
            device.free_memory(self.memory, None);
            self.memory = vk::DeviceMemory::null();
        }
    }
}

/// Descriptor set layout + pool + allocated set.
#[derive(Clone, Copy, Default)]
pub struct DescriptorAlloc {
    pub layout: vk::DescriptorSetLayout,
    pub pool: vk::DescriptorPool,
    pub set: vk::DescriptorSet,
}

impl DescriptorAlloc {
    #[inline]
    pub const fn is_null(&self) -> bool {
        self.layout == vk::DescriptorSetLayout::null()
    }

    #[inline]
    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        if self.pool != vk::DescriptorPool::null() {
            device.destroy_descriptor_pool(self.pool, None);
            self.pool = vk::DescriptorPool::null();
        }
        if self.layout != vk::DescriptorSetLayout::null() {
            device.destroy_descriptor_set_layout(self.layout, None);
            self.layout = vk::DescriptorSetLayout::null();
        }
        self.set = vk::DescriptorSet::null();
    }
}

/// Persistent upload context: fence-based immediate submits without queue_wait_idle
/// and without per-call command buffer allocations.
#[derive(Clone, Copy, Default)]
pub struct UploadCtx {
    pub pool: vk::CommandPool,
    pub cmd: vk::CommandBuffer,
    pub fence: vk::Fence,
}

impl UploadCtx {
    #[inline]
    pub const fn is_ready(&self) -> bool {
        self.pool != vk::CommandPool::null()
            && self.cmd != vk::CommandBuffer::null()
            && self.fence != vk::Fence::null()
    }

    #[inline]
    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        if self.pool != vk::CommandPool::null() {
            if self.cmd != vk::CommandBuffer::null() {
                device.free_command_buffers(self.pool, &[self.cmd]);
                self.cmd = vk::CommandBuffer::null();
            }
            device.destroy_command_pool(self.pool, None);
            self.pool = vk::CommandPool::null();
        }
        if self.fence != vk::Fence::null() {
            device.destroy_fence(self.fence, None);
            self.fence = vk::Fence::null();
        }
    }

    #[inline]
    pub unsafe fn submit<F: FnOnce(vk::CommandBuffer)>(
        &self,
        device: &ash::Device,
        queue: vk::Queue,
        f: F,
    ) -> VkResult<()> {
        debug_assert!(self.is_ready());

        device.reset_fences(&[self.fence])?;
        device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())?;

        device.begin_command_buffer(
            self.cmd,
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )?;

        f(self.cmd);

        device.end_command_buffer(self.cmd)?;

        let submit = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&self.cmd));
        device.queue_submit(queue, std::slice::from_ref(&submit), self.fence)?;
        device.wait_for_fences(&[self.fence], true, u64::MAX)?;

        Ok(())
    }
}