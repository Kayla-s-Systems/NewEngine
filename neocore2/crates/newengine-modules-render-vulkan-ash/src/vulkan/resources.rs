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
    pub fn is_null(&self) -> bool {
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
    pub fn is_null(&self) -> bool {
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
    pub fn is_null(&self) -> bool {
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
    pub fn is_ready(&self) -> bool {
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
    pub unsafe fn is_in_flight(&self, device: &ash::Device) -> VkResult<bool> {
        debug_assert!(self.is_ready());
        match device.get_fence_status(self.fence) {
            Ok(_) => Ok(false),
            Err(vk::Result::NOT_READY) => Ok(true),
            Err(e) => Err(e.into()),
        }
    }

    /// Records and submits an upload command buffer.
    ///
    /// Contract:
    /// - This method does NOT block.
    /// - The caller must ensure that the context is not in flight (or accept a wait).
    ///
    /// Returns the fence associated with this submission.
    #[inline]
    pub unsafe fn submit_async<F: FnOnce(vk::CommandBuffer)>(
        &self,
        device: &ash::Device,
        queue: vk::Queue,
        f: F,
    ) -> VkResult<vk::Fence> {
        debug_assert!(self.is_ready());

        // If the context is still in flight, we must wait; otherwise we'd reset in-use resources.
        if self.is_in_flight(device)? {
            device.wait_for_fences(&[self.fence], true, u64::MAX)?;
        }

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

        Ok(self.fence)
    }
}

/// Deferred destruction queue keyed by a fence.
///
/// This is the minimal "game-ready" primitive for upload staging cleanup.
/// Anything pushed here MUST remain valid until the corresponding fence is signaled.
pub struct DeferredFree {
    items: Vec<DeferredItem>,
}

impl DeferredFree {
    #[inline]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline]
    pub fn push_buffer(&mut self, fence: vk::Fence, buffer: vk::Buffer, memory: vk::DeviceMemory) {
        if buffer == vk::Buffer::null() && memory == vk::DeviceMemory::null() {
            return;
        }
        self.items.push(DeferredItem::Buffer {
            fence,
            buffer,
            memory,
        });
    }

    #[inline]
    pub fn push_descriptor_pool(&mut self, fence: vk::Fence, pool: vk::DescriptorPool) {
        if pool == vk::DescriptorPool::null() {
            return;
        }
        self.items
            .push(DeferredItem::DescriptorPool { fence, pool });
    }

    #[inline]
    pub fn push_image(
        &mut self,
        fence: vk::Fence,
        image: vk::Image,
        view: vk::ImageView,
        memory: vk::DeviceMemory,
        sampler: vk::Sampler,
    ) {
        if image == vk::Image::null()
            && view == vk::ImageView::null()
            && memory == vk::DeviceMemory::null()
            && sampler == vk::Sampler::null()
        {
            return;
        }
        self.items.push(DeferredItem::Image {
            fence,
            image,
            view,
            memory,
            sampler,
        });
    }

    /// Destroys everything whose fence is already signaled.
    pub unsafe fn pump(&mut self, device: &ash::Device) -> VkResult<()> {
        let mut i = 0usize;
        while i < self.items.len() {
            let fence = self.items[i].fence();
            let signaled = match device.get_fence_status(fence) {
                Ok(_) => true,
                Err(vk::Result::NOT_READY) => false,
                Err(e) => return Err(e.into()),
            };

            if !signaled {
                i += 1;
                continue;
            }

            let item = self.items.swap_remove(i);
            item.destroy(device);
        }
        Ok(())
    }
}

enum DeferredItem {
    Buffer {
        fence: vk::Fence,
        buffer: vk::Buffer,
        memory: vk::DeviceMemory,
    },
    DescriptorPool {
        fence: vk::Fence,
        pool: vk::DescriptorPool,
    },
    Image {
        fence: vk::Fence,
        image: vk::Image,
        view: vk::ImageView,
        memory: vk::DeviceMemory,
        sampler: vk::Sampler,
    },
}

impl DeferredItem {
    #[inline]
    fn fence(&self) -> vk::Fence {
        match *self {
            DeferredItem::Buffer { fence, .. } => fence,
            DeferredItem::DescriptorPool { fence, .. } => fence,
            DeferredItem::Image { fence, .. } => fence,
        }
    }

    #[inline]
    unsafe fn destroy(self, device: &ash::Device) {
        match self {
            DeferredItem::Buffer { buffer, memory, .. } => {
                if buffer != vk::Buffer::null() {
                    device.destroy_buffer(buffer, None);
                }
                if memory != vk::DeviceMemory::null() {
                    device.free_memory(memory, None);
                }
            }
            DeferredItem::DescriptorPool { pool, .. } => {
                if pool != vk::DescriptorPool::null() {
                    device.destroy_descriptor_pool(pool, None);
                }
            }
            DeferredItem::Image {
                image,
                view,
                memory,
                sampler,
                ..
            } => {
                if sampler != vk::Sampler::null() {
                    device.destroy_sampler(sampler, None);
                }
                if view != vk::ImageView::null() {
                    device.destroy_image_view(view, None);
                }
                if image != vk::Image::null() {
                    device.destroy_image(image, None);
                }
                if memory != vk::DeviceMemory::null() {
                    device.free_memory(memory, None);
                }
            }
        }
    }
}
