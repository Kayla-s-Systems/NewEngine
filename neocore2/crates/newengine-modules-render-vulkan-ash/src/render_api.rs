use crate::vulkan::pipeline::create_shader_module;
use crate::vulkan::util::immediate_submit;
use crate::vulkan::VulkanRenderer;

use ash::vk;

use newengine_core::render::*;
use newengine_core::{EngineError, EngineResult};
use newengine_ui::draw::UiDrawList;
use newengine_ui::texture::reserved as ui_reserved;

use newengine_ui::draw::UiTexId;
use std::collections::HashMap;
use std::ffi::CString;


#[derive(Clone, Copy)]
struct VkBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    host_visible: bool,
}

#[derive(Clone)]
struct VkShader {
    module: vk::ShaderModule,
    stage: vk::ShaderStageFlags,
    entry: CString,
}

#[derive(Clone)]
struct VkBgLayout {
    layout: vk::DescriptorSetLayout,
    bindings: Vec<BindingKind>,
}

#[derive(Clone, Copy)]
struct VkBindGroup {
    set: vk::DescriptorSet,
    pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
}

#[derive(Clone, Copy)]
struct VkPipeline {
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
}

enum RecordedCmd {
    SetViewport(vk::Viewport),
    SetScissor(vk::Rect2D),
    BindPipeline(vk::Pipeline),
    BindDescriptorSets {
        layout: vk::PipelineLayout,
        first_set: u32,
        sets: [vk::DescriptorSet; 4],
        set_count: u32,
    },
    BindVertexBuffer {
        first_binding: u32,
        buffers: [vk::Buffer; 4],
        offsets: [vk::DeviceSize; 4],
        count: u32,
    },
    BindIndexBuffer {
        buffer: vk::Buffer,
        offset: vk::DeviceSize,
        index_type: vk::IndexType,
    },
    Draw(DrawArgs),
    DrawIndexed(DrawIndexedArgs),
}

pub struct VulkanRenderApi {
    renderer: VulkanRenderer,
    target: Extent2D,

    active_render_target: Option<u32>,

    next_id: u32,

    buffers: HashMap<BufferId, VkBuffer>,
    shaders: HashMap<ShaderId, VkShader>,
    bg_layouts: HashMap<BindGroupLayoutId, VkBgLayout>,
    bind_groups: HashMap<BindGroupId, VkBindGroup>,
    pipelines: HashMap<PipelineId, VkPipeline>,

    current_pipeline: Option<PipelineId>,
    current_vertex: [Option<BufferSlice>; 4],
    current_index: Option<(BufferSlice, IndexFormat)>,
    current_bind_groups: [Option<BindGroupId>; 4],

    recorded: Vec<RecordedCmd>,
}

impl VulkanRenderApi {
    #[inline]
    pub fn new(renderer: VulkanRenderer, width: u32, height: u32) -> Self {
        Self {
            renderer,
            target: Extent2D::new(width, height),
            active_render_target: None,
            next_id: 1,
            buffers: HashMap::new(),
            shaders: HashMap::new(),
            bg_layouts: HashMap::new(),
            bind_groups: HashMap::new(),
            pipelines: HashMap::new(),
            current_pipeline: None,
            current_vertex: [None, None, None, None],
            current_index: None,
            current_bind_groups: [None, None, None, None],
            recorded: Vec::new(),
        }
    }

    #[inline]
    pub fn set_ui_draw_list(&mut self, ui: UiDrawList) {
        self.renderer.set_ui_draw_list(ui);
    }

    #[inline]
    fn alloc_u32(&mut self) -> u32 {
        let v = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        v
    }

    #[inline]
    fn err<T>(&self, msg: impl Into<String>) -> EngineResult<T> {
        Err(EngineError::other(msg.into()))
    }

    #[inline]
    fn map_stage(stage: ShaderStage) -> vk::ShaderStageFlags {
        match stage {
            ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
            ShaderStage::Fragment => vk::ShaderStageFlags::FRAGMENT,
            ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
        }
    }

    #[inline]
    fn map_topology(t: PrimitiveTopology) -> vk::PrimitiveTopology {
        match t {
            PrimitiveTopology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
            PrimitiveTopology::TriangleStrip => vk::PrimitiveTopology::TRIANGLE_STRIP,
            PrimitiveTopology::LineList => vk::PrimitiveTopology::LINE_LIST,
            PrimitiveTopology::LineStrip => vk::PrimitiveTopology::LINE_STRIP,
        }
    }

    #[inline]
    fn map_index_format(f: IndexFormat) -> vk::IndexType {
        match f {
            IndexFormat::U16 => vk::IndexType::UINT16,
            IndexFormat::U32 => vk::IndexType::UINT32,
        }
    }

    #[inline]
    fn map_vertex_format(f: VertexFormat) -> vk::Format {
        match f {
            VertexFormat::Float32x2 => vk::Format::R32G32_SFLOAT,
            VertexFormat::Float32x3 => vk::Format::R32G32B32_SFLOAT,
            VertexFormat::Float32x4 => vk::Format::R32G32B32A32_SFLOAT,
            VertexFormat::Unorm8x4 => vk::Format::R8G8B8A8_UNORM,
        }
    }

    fn buffer_usage_flags(u: BufferUsage) -> vk::BufferUsageFlags {
        match u {
            BufferUsage::Vertex => vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            BufferUsage::Index => vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            BufferUsage::Uniform => vk::BufferUsageFlags::UNIFORM_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            BufferUsage::Storage => vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            BufferUsage::Staging => vk::BufferUsageFlags::TRANSFER_SRC,
        }
    }

    fn memory_props(h: MemoryHint) -> vk::MemoryPropertyFlags {
        match h {
            MemoryHint::GpuOnly => vk::MemoryPropertyFlags::DEVICE_LOCAL,
            MemoryHint::CpuToGpu => vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            MemoryHint::GpuToCpu => vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        }
    }

    unsafe fn find_memory_type(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        type_bits: u32,
        props: vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        let mem = instance.get_physical_device_memory_properties(physical_device);
        for i in 0..mem.memory_type_count {
            let mt = mem.memory_types[i as usize];
            if (type_bits & (1u32 << i)) != 0 && mt.property_flags.contains(props) {
                return Some(i);
            }
        }
        None
    }

    unsafe fn create_vk_buffer(
        &self,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        props: vk::MemoryPropertyFlags,
    ) -> EngineResult<VkBuffer> {
        let device = &self.renderer.core.device;

        let info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = device
            .create_buffer(&info, None)
            .map_err(|e| EngineError::other(e.to_string()))?;

        let req = device.get_buffer_memory_requirements(buffer);

        let mem_type = Self::find_memory_type(
            &self.renderer.core.instance,
            self.renderer.core.physical_device,
            req.memory_type_bits,
            props,
        )
            .ok_or_else(|| EngineError::other("No compatible Vulkan memory type"))?;

        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(req.size)
            .memory_type_index(mem_type);

        let memory = device
            .allocate_memory(&alloc, None)
            .map_err(|e| EngineError::other(e.to_string()))?;

        device
            .bind_buffer_memory(buffer, memory, 0)
            .map_err(|e| EngineError::other(e.to_string()))?;

        Ok(VkBuffer {
            buffer,
            memory,
            size,
            usage,
            host_visible: props.contains(vk::MemoryPropertyFlags::HOST_VISIBLE),
        })
    }

    unsafe fn current_cmd(&self) -> Option<vk::CommandBuffer> {
        if !self.renderer.debug.in_frame {
            return None;
        }
        let idx = self.renderer.debug.current_swapchain_idx;
        Some(self.renderer.frames.command_buffers[idx])
    }

    unsafe fn flush_recorded(&mut self) -> EngineResult<()> {
        let Some(cmd) = self.current_cmd() else {
            self.recorded.clear();
            return Ok(());
        };

        // Ensure we are inside the correct render pass before issuing any graphics commands.
        // This is a hard requirement in Vulkan: draw calls outside a render pass are invalid.
        if let Some(rt) = self.active_render_target {
            self.renderer
                .begin_render_target_pass(cmd, rt, None)
                .map_err(|e| EngineError::other(e.to_string()))?;
        } else {
            self.renderer
                .ensure_swapchain_pass(cmd)
                .map_err(|e| EngineError::other(e.to_string()))?;
        }

        let device = &self.renderer.core.device;

        for c in self.recorded.drain(..) {
            match c {
                RecordedCmd::SetViewport(vp) => device.cmd_set_viewport(cmd, 0, std::slice::from_ref(&vp)),
                RecordedCmd::SetScissor(sc) => device.cmd_set_scissor(cmd, 0, std::slice::from_ref(&sc)),
                RecordedCmd::BindPipeline(p) => device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, p),
                RecordedCmd::BindDescriptorSets { layout, first_set, sets, set_count } => {
                    device.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        layout,
                        first_set,
                        &sets[..set_count as usize],
                        &[],
                    );
                }
                RecordedCmd::BindVertexBuffer { first_binding, buffers, offsets, count } => {
                    device.cmd_bind_vertex_buffers(
                        cmd,
                        first_binding,
                        &buffers[..count as usize],
                        &offsets[..count as usize],
                    );
                }
                RecordedCmd::BindIndexBuffer { buffer, offset, index_type } => {
                    device.cmd_bind_index_buffer(cmd, buffer, offset, index_type);
                }
                RecordedCmd::Draw(a) => device.cmd_draw(cmd, a.vertex_count, a.instance_count, a.first_vertex, a.first_instance),
                RecordedCmd::DrawIndexed(a) => device.cmd_draw_indexed(
                    cmd,
                    a.index_count,
                    a.instance_count,
                    a.first_index,
                    a.vertex_offset,
                    a.first_instance,
                ),
            }
        }

        Ok(())
    }
}

impl Drop for VulkanRenderApi {
    fn drop(&mut self) {
        unsafe {
            let device = &self.renderer.core.device;

            for (_, p) in self.pipelines.drain() {
                if p.pipeline != vk::Pipeline::null() {
                    device.destroy_pipeline(p.pipeline, None);
                }
                if p.layout != vk::PipelineLayout::null() {
                    device.destroy_pipeline_layout(p.layout, None);
                }
            }

            for (_, bg) in self.bind_groups.drain() {
                if bg.pool != vk::DescriptorPool::null() {
                    device.destroy_descriptor_pool(bg.pool, None);
                }
                let _ = bg.layout;
            }

            for (_, l) in self.bg_layouts.drain() {
                if l.layout != vk::DescriptorSetLayout::null() {
                    device.destroy_descriptor_set_layout(l.layout, None);
                }
            }

            for (_, s) in self.shaders.drain() {
                if s.module != vk::ShaderModule::null() {
                    device.destroy_shader_module(s.module, None);
                }
            }

            for (_, b) in self.buffers.drain() {
                if b.buffer != vk::Buffer::null() {
                    device.destroy_buffer(b.buffer, None);
                }
                if b.memory != vk::DeviceMemory::null() {
                    device.free_memory(b.memory, None);
                }
                let _ = b.size;
            }
        }
    }
}

impl RenderApi for VulkanRenderApi {
    fn begin_frame(&mut self, desc: BeginFrameDesc) -> EngineResult<()> {
        self.recorded.clear();
        self.current_pipeline = None;
        self.current_vertex = [None, None, None, None];
        self.current_index = None;
        self.current_bind_groups = [None, None, None, None];
        self.active_render_target = None;

        self.renderer.begin_frame(desc.clear_color).map_err(|e| EngineError::other(e.to_string()))
    }

    #[inline]
    fn set_ui_draw_list(&mut self, ui: UiDrawList) {
        self.renderer.set_ui_draw_list(ui);
    }

    fn end_frame(&mut self) -> EngineResult<()> {
        unsafe {
            self.flush_recorded()?;
        }
        self.renderer.end_frame().map_err(|e| EngineError::other(e.to_string()))
    }

    fn resize(&mut self, width: u32, height: u32) -> EngineResult<()> {
        self.target = Extent2D::new(width, height);
        self.renderer.resize(width, height).map_err(|e| EngineError::other(e.to_string()))
    }


    fn create_render_target(&mut self, desc: RenderTargetDesc) -> EngineResult<RenderTargetId> {
        if desc.extent.width == 0 || desc.extent.height == 0 {
            return Err(EngineError::other("create_render_target: zero extent"));
        }

        if desc.depth.is_some() {
            // Current Vulkan backend uses a single-color render pass.
            // Depth-stencil support will be added via a dedicated render pass and attachment set.
            return Err(EngineError::other(
                "create_render_target: depth is not supported by Vulkan backend yet",
            ));
        }

        let id_u32 = self.alloc_u32();
        let id = RenderTargetId::new(id_u32);

        let extent = vk::Extent2D {
            width: desc.extent.width,
            height: desc.extent.height,
        };

        self.renderer
            .create_render_target(id_u32, extent)
            .map_err(|e| EngineError::other(e.to_string()))?;

        Ok(id)
    }

    fn destroy_render_target(&mut self, id: RenderTargetId) {
        // If user destroys the active target, force scope reset.
        if self.active_render_target == Some(id.0.get()) {
            self.active_render_target = None;
            self.current_pipeline = None;
            self.current_vertex = [None, None, None, None];
            self.current_index = None;
            self.current_bind_groups = [None, None, None, None];
            self.recorded.clear();
        }

        unsafe {
            self.renderer.destroy_render_target(id.0.get());
        }
    }

    fn render_target_ui_tex_id(&self, id: RenderTargetId) -> EngineResult<UiTexId> {
        let key = id.0.get();
        if !self.renderer.render_targets.contains_key(&key) {
            return Err(EngineError::other("render_target_ui_tex_id: unknown render target"));
        }

        Ok(ui_reserved::external_from_u32(key))
    }

    fn begin_render_target(&mut self, desc: BeginRenderTargetDesc) -> EngineResult<()> {
        unsafe {
            // Flush any pending commands recorded for the previous target.
            self.flush_recorded()?;

            let Some(cmd) = self.current_cmd() else {
                return Ok(());
            };

            self.renderer
                .begin_render_target_pass(cmd, desc.target.0.get(), desc.clear_color)
                .map_err(|e| EngineError::other(e.to_string()))?;
        }

        self.active_render_target = Some(desc.target.0.get());

        // Pipeline state must be rebound after switching render pass scope.
        self.current_pipeline = None;
        self.current_vertex = [None, None, None, None];
        self.current_index = None;
        self.current_bind_groups = [None, None, None, None];
        Ok(())
    }

    fn end_render_target(&mut self) -> EngineResult<()> {
        unsafe {
            self.flush_recorded()?;

            let Some(cmd) = self.current_cmd() else {
                self.active_render_target = None;
                return Ok(());
            };

            // Close the currently active pass (render target) and transition for sampling.
            self.renderer
                .end_active_pass(cmd)
                .map_err(|e| EngineError::other(e.to_string()))?;
        }

        self.active_render_target = None;

        // Force rebind; next flush will open swapchain pass on demand.
        self.current_pipeline = None;
        self.current_vertex = [None, None, None, None];
        self.current_index = None;
        self.current_bind_groups = [None, None, None, None];
        Ok(())
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> EngineResult<BufferId> {
        let id = BufferId::new(self.alloc_u32());
        unsafe {
            let usage = Self::buffer_usage_flags(desc.usage);
            let props = Self::memory_props(desc.memory);
            let b = self.create_vk_buffer(desc.size as vk::DeviceSize, usage, props)?;
            self.buffers.insert(id, b);
        }
        Ok(id)
    }

    fn destroy_buffer(&mut self, id: BufferId) {
        if let Some(b) = self.buffers.remove(&id) {
            unsafe {
                let device = &self.renderer.core.device;
                if b.buffer != vk::Buffer::null() {
                    device.destroy_buffer(b.buffer, None);
                }
                if b.memory != vk::DeviceMemory::null() {
                    device.free_memory(b.memory, None);
                }
            }
        }
    }

    fn write_buffer(&mut self, id: BufferId, offset: u64, data: &[u8]) -> EngineResult<()> {
        let b = *self
            .buffers
            .get(&id)
            .ok_or_else(|| EngineError::other("write_buffer: invalid BufferId"))?;

        if (offset as u128) + (data.len() as u128) > (b.size as u128) {
            return Err(EngineError::other("write_buffer: out of bounds"));
        }

        unsafe {
            let device = &self.renderer.core.device;

            if b.host_visible {
                let ptr = device
                    .map_memory(
                        b.memory,
                        offset as vk::DeviceSize,
                        data.len() as vk::DeviceSize,
                        vk::MemoryMapFlags::empty(),
                    )
                    .map_err(|e| EngineError::other(e.to_string()))? as *mut u8;

                std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
                device.unmap_memory(b.memory);
                return Ok(());
            }

            let staging = self.create_vk_buffer(
                data.len() as vk::DeviceSize,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;

            let ptr = device
                .map_memory(
                    staging.memory,
                    0,
                    data.len() as vk::DeviceSize,
                    vk::MemoryMapFlags::empty(),
                )
                .map_err(|e| EngineError::other(e.to_string()))? as *mut u8;

            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
            device.unmap_memory(staging.memory);

            immediate_submit(
                device,
                self.renderer.frames.upload_command_pool,
                self.renderer.core.queue,
                |cmd| {
                    let region = vk::BufferCopy::default()
                        .src_offset(0)
                        .dst_offset(offset as vk::DeviceSize)
                        .size(data.len() as vk::DeviceSize);

                    device.cmd_copy_buffer(cmd, staging.buffer, b.buffer, std::slice::from_ref(&region));

                    let (dst_stage, dst_access) = if b.usage.intersects(
                        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
                    ) {
                        (
                            vk::PipelineStageFlags::VERTEX_INPUT,
                            vk::AccessFlags::VERTEX_ATTRIBUTE_READ | vk::AccessFlags::INDEX_READ,
                        )
                    } else if b.usage.contains(vk::BufferUsageFlags::UNIFORM_BUFFER) {
                        (
                            vk::PipelineStageFlags::VERTEX_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER,
                            vk::AccessFlags::UNIFORM_READ,
                        )
                    } else if b.usage.contains(vk::BufferUsageFlags::STORAGE_BUFFER) {
                        (
                            vk::PipelineStageFlags::VERTEX_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER,
                            vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
                        )
                    } else {
                        (
                            vk::PipelineStageFlags::ALL_COMMANDS,
                            vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
                        )
                    };

                    let barrier = vk::BufferMemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .dst_access_mask(dst_access)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .buffer(b.buffer)
                        .offset(offset as vk::DeviceSize)
                        .size(data.len() as vk::DeviceSize);

                    device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::TRANSFER,
                        dst_stage,
                        vk::DependencyFlags::empty(),
                        &[],
                        std::slice::from_ref(&barrier),
                        &[],
                    );
                },
            )
                .map_err(|e| EngineError::other(e.to_string()))?;

            device.destroy_buffer(staging.buffer, None);
            device.free_memory(staging.memory, None);
        }

        Ok(())
    }

    fn create_texture(&mut self, _desc: TextureDesc) -> EngineResult<TextureId> {
        self.err("VulkanRenderApi: create_texture not implemented (world textures pending)")
    }

    fn destroy_texture(&mut self, _id: TextureId) {}

    fn create_sampler(&mut self, _desc: SamplerDesc) -> EngineResult<SamplerId> {
        self.err("VulkanRenderApi: create_sampler not implemented (world samplers pending)")
    }

    fn destroy_sampler(&mut self, _id: SamplerId) {}

    fn create_shader(&mut self, desc: ShaderDesc) -> EngineResult<ShaderId> {
        let id = ShaderId::new(self.alloc_u32());

        unsafe {
            let bytes: &[u8] = bytemuck::cast_slice(&desc.spirv);

            let module = create_shader_module(&self.renderer.core.device, bytes)
                .map_err(|e: crate::error::VkRenderError| EngineError::other(e.to_string()))?;

            let stage = Self::map_stage(desc.stage);

            let entry = CString::new(desc.entry)
                .map_err(|_| EngineError::other("ShaderDesc.entry must be a valid C string"))?;

            self.shaders.insert(id, VkShader { module, stage, entry });
        }

        Ok(id)
    }

    fn destroy_shader(&mut self, id: ShaderId) {
        if let Some(s) = self.shaders.remove(&id) {
            unsafe { self.renderer.core.device.destroy_shader_module(s.module, None); }
        }
    }

    fn create_pipeline(&mut self, desc: PipelineDesc) -> EngineResult<PipelineId> {
        let id = PipelineId::new(self.alloc_u32());

        let vs = self.shaders.get(&desc.vs).ok_or_else(|| EngineError::other("create_pipeline: invalid vs"))?.clone();
        let fs = self.shaders.get(&desc.fs).ok_or_else(|| EngineError::other("create_pipeline: invalid fs"))?.clone();

        let mut set_layouts: Vec<vk::DescriptorSetLayout> = Vec::with_capacity(desc.bind_group_layouts.len());
        for l_id in &desc.bind_group_layouts {
            let l = self.bg_layouts.get(l_id).ok_or_else(|| EngineError::other("create_pipeline: invalid bind group layout"))?;
            set_layouts.push(l.layout);
        }

        unsafe {
            let device = &self.renderer.core.device;

            let layout_ci = vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts);
            let layout = device.create_pipeline_layout(&layout_ci, None).map_err(|e| EngineError::other(e.to_string()))?;

            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vs.stage).module(vs.module).name(&vs.entry),
                vk::PipelineShaderStageCreateInfo::default().stage(fs.stage).module(fs.module).name(&fs.entry),
            ];

            let mut binding_descs: Vec<vk::VertexInputBindingDescription> = Vec::new();
            let mut attr_descs: Vec<vk::VertexInputAttributeDescription> = Vec::new();

            for (i, l) in desc.vertex_layouts.iter().enumerate() {
                binding_descs.push(
                    vk::VertexInputBindingDescription::default()
                        .binding(i as u32)
                        .stride(l.stride)
                        .input_rate(vk::VertexInputRate::VERTEX),
                );

                for a in &l.attributes {
                    attr_descs.push(
                        vk::VertexInputAttributeDescription::default()
                            .binding(i as u32)
                            .location(a.location)
                            .format(Self::map_vertex_format(a.format))
                            .offset(a.offset),
                    );
                }
            }

            let vi = vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(&binding_descs)
                .vertex_attribute_descriptions(&attr_descs);

            let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(Self::map_topology(desc.topology));
            let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);

            let rs = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL)
                .cull_mode(vk::CullModeFlags::BACK)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .line_width(1.0);

            let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);

            let ca = vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(false)
                .color_write_mask(
                    vk::ColorComponentFlags::R
                        | vk::ColorComponentFlags::G
                        | vk::ColorComponentFlags::B
                        | vk::ColorComponentFlags::A,
                );

            let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(std::slice::from_ref(&ca));

            let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let ds = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);

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
                .render_pass(self.renderer.pipelines.render_pass)
                .subpass(0);

            let pipelines = device.create_graphics_pipelines(vk::PipelineCache::null(), &[gp], None);
            let pipeline = match pipelines {
                Ok(v) => v[0],
                Err((_, e)) => return Err(EngineError::other(e.to_string())),
            };

            self.pipelines.insert(id, VkPipeline { pipeline, layout });
        }

        Ok(id)
    }

    fn destroy_pipeline(&mut self, id: PipelineId) {
        if let Some(p) = self.pipelines.remove(&id) {
            unsafe {
                let device = &self.renderer.core.device;
                if p.pipeline != vk::Pipeline::null() {
                    device.destroy_pipeline(p.pipeline, None);
                }
                if p.layout != vk::PipelineLayout::null() {
                    device.destroy_pipeline_layout(p.layout, None);
                }
            }
        }
    }

    fn create_bind_group_layout(&mut self, desc: BindGroupLayoutDesc) -> EngineResult<BindGroupLayoutId> {
        let id = BindGroupLayoutId::new(self.alloc_u32());

        unsafe {
            let device = &self.renderer.core.device;

            let mut vk_bindings: Vec<vk::DescriptorSetLayoutBinding> = Vec::with_capacity(desc.bindings.len());
            for (i, k) in desc.bindings.iter().enumerate() {
                let ty = match k {
                    BindingKind::Texture2D => vk::DescriptorType::SAMPLED_IMAGE,
                    BindingKind::Sampler => vk::DescriptorType::SAMPLER,
                    BindingKind::UniformBuffer => vk::DescriptorType::UNIFORM_BUFFER,
                    BindingKind::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
                };

                vk_bindings.push(
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(i as u32)
                        .descriptor_type(ty)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
                );
            }

            let ci = vk::DescriptorSetLayoutCreateInfo::default().bindings(&vk_bindings);
            let layout = device
                .create_descriptor_set_layout(&ci, None)
                .map_err(|e| EngineError::other(e.to_string()))?;

            self.bg_layouts.insert(id, VkBgLayout { layout, bindings: desc.bindings });
        }

        Ok(id)
    }

    fn destroy_bind_group_layout(&mut self, id: BindGroupLayoutId) {
        if let Some(l) = self.bg_layouts.remove(&id) {
            unsafe { self.renderer.core.device.destroy_descriptor_set_layout(l.layout, None); }
        }
    }

    fn create_bind_group(&mut self, desc: BindGroupDesc) -> EngineResult<BindGroupId> {
        let id = BindGroupId::new(self.alloc_u32());
        let l = self
            .bg_layouts
            .get(&desc.layout)
            .ok_or_else(|| EngineError::other("create_bind_group: invalid layout"))?
            .clone();

        unsafe {
            let device = &self.renderer.core.device;

            let mut need_img = 0u32;
            let mut need_samp = 0u32;
            let mut need_ubo = 0u32;
            let mut need_ssbo = 0u32;

            for k in &l.bindings {
                match k {
                    BindingKind::Texture2D => need_img += 1,
                    BindingKind::Sampler => need_samp += 1,
                    BindingKind::UniformBuffer => need_ubo += 1,
                    BindingKind::StorageBuffer => need_ssbo += 1,
                }
            }

            let mut pool_sizes: Vec<vk::DescriptorPoolSize> = Vec::new();
            if need_img > 0 {
                pool_sizes.push(
                    vk::DescriptorPoolSize::default()
                        .ty(vk::DescriptorType::SAMPLED_IMAGE)
                        .descriptor_count(need_img),
                );
            }
            if need_samp > 0 {
                pool_sizes.push(
                    vk::DescriptorPoolSize::default()
                        .ty(vk::DescriptorType::SAMPLER)
                        .descriptor_count(need_samp),
                );
            }
            if need_ubo > 0 {
                pool_sizes.push(
                    vk::DescriptorPoolSize::default()
                        .ty(vk::DescriptorType::UNIFORM_BUFFER)
                        .descriptor_count(need_ubo),
                );
            }
            if need_ssbo > 0 {
                pool_sizes.push(
                    vk::DescriptorPoolSize::default()
                        .ty(vk::DescriptorType::STORAGE_BUFFER)
                        .descriptor_count(need_ssbo),
                );
            }

            let pool_ci = vk::DescriptorPoolCreateInfo::default()
                .max_sets(1)
                .pool_sizes(&pool_sizes);

            let pool = device
                .create_descriptor_pool(&pool_ci, None)
                .map_err(|e| EngineError::other(e.to_string()))?;

            let set_layouts = [l.layout];
            let alloc = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(pool)
                .set_layouts(&set_layouts);

            let set = device
                .allocate_descriptor_sets(&alloc)
                .map_err(|e| EngineError::other(e.to_string()))?[0];

            let mut writes: Vec<vk::WriteDescriptorSet> = Vec::new();
            let mut buf_infos: Vec<vk::DescriptorBufferInfo> = Vec::new();

            #[derive(Clone, Copy)]
            struct PendingBufWrite {
                binding: u32,
                ty: vk::DescriptorType,
                buf_info_index: usize,
            }

            let mut pending: Vec<PendingBufWrite> = Vec::new();

            buf_infos.reserve_exact((need_ubo + need_ssbo) as usize);
            pending.reserve_exact((need_ubo + need_ssbo) as usize);

            for (binding, k) in l.bindings.iter().enumerate() {
                match k {
                    BindingKind::UniformBuffer => {
                        let Some(bb) = desc.uniform0 else { continue; };
                        let b = *self
                            .buffers
                            .get(&bb.buffer)
                            .ok_or_else(|| EngineError::other("create_bind_group: invalid uniform0 buffer"))?;

                        buf_infos.push(
                            vk::DescriptorBufferInfo::default()
                                .buffer(b.buffer)
                                .offset(bb.offset)
                                .range(bb.size),
                        );

                        pending.push(PendingBufWrite {
                            binding: binding as u32,
                            ty: vk::DescriptorType::UNIFORM_BUFFER,
                            buf_info_index: buf_infos.len() - 1,
                        });
                    }
                    BindingKind::StorageBuffer => {
                        let Some(bb) = desc.storage0 else { continue; };
                        let b = *self
                            .buffers
                            .get(&bb.buffer)
                            .ok_or_else(|| EngineError::other("create_bind_group: invalid storage0 buffer"))?;

                        buf_infos.push(
                            vk::DescriptorBufferInfo::default()
                                .buffer(b.buffer)
                                .offset(bb.offset)
                                .range(bb.size),
                        );

                        pending.push(PendingBufWrite {
                            binding: binding as u32,
                            ty: vk::DescriptorType::STORAGE_BUFFER,
                            buf_info_index: buf_infos.len() - 1,
                        });
                    }
                    BindingKind::Texture2D => {
                        return Err(EngineError::other(
                            "create_bind_group: Texture2D not implemented (world textures pending)",
                        ));
                    }
                    BindingKind::Sampler => {
                        return Err(EngineError::other(
                            "create_bind_group: Sampler not implemented (world samplers pending)",
                        ));
                    }
                }
            }

            writes.reserve_exact(pending.len());
            for p in pending {
                let bi_ref = std::slice::from_ref(&buf_infos[p.buf_info_index]);
                writes.push(
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(p.binding)
                        .descriptor_type(p.ty)
                        .buffer_info(bi_ref),
                );
            }

            if !writes.is_empty() {
                device.update_descriptor_sets(&writes, &[]);
            }

            self.bind_groups.insert(
                id,
                VkBindGroup {
                    set,
                    pool,
                    layout: l.layout,
                },
            );
        }

        Ok(id)
    }

    fn destroy_bind_group(&mut self, id: BindGroupId) {
        if let Some(bg) = self.bind_groups.remove(&id) {
            unsafe {
                if bg.pool != vk::DescriptorPool::null() {
                    self.renderer.core.device.destroy_descriptor_pool(bg.pool, None);
                }
            }
        }
    }

    fn set_viewport(&mut self, vp: Viewport) -> EngineResult<()> {
        let vk_vp = vk::Viewport {
            x: vp.x,
            y: vp.y,
            width: vp.w,
            height: vp.h,
            min_depth: vp.min_depth,
            max_depth: vp.max_depth,
        };
        self.recorded.push(RecordedCmd::SetViewport(vk_vp));
        Ok(())
    }

    fn set_scissor(&mut self, rect: RectI32) -> EngineResult<()> {
        let sc = vk::Rect2D {
            offset: vk::Offset2D { x: rect.x, y: rect.y },
            extent: vk::Extent2D { width: rect.w.max(0) as u32, height: rect.h.max(0) as u32 },
        };
        self.recorded.push(RecordedCmd::SetScissor(sc));
        Ok(())
    }

    fn set_pipeline(&mut self, pipeline: PipelineId) -> EngineResult<()> {
        let p = *self.pipelines.get(&pipeline).ok_or_else(|| EngineError::other("set_pipeline: invalid PipelineId"))?;
        self.current_pipeline = Some(pipeline);
        self.recorded.push(RecordedCmd::BindPipeline(p.pipeline));
        Ok(())
    }

    fn set_bind_group(&mut self, index: u32, group: BindGroupId) -> EngineResult<()> {
        if index as usize >= self.current_bind_groups.len() {
            return self.err("set_bind_group: index out of range (max 4)");
        }
        self.current_bind_groups[index as usize] = Some(group);
        Ok(())
    }

    fn set_vertex_buffer(&mut self, slot: u32, slice: BufferSlice) -> EngineResult<()> {
        if slot as usize >= self.current_vertex.len() {
            return self.err("set_vertex_buffer: slot out of range (max 4)");
        }
        self.current_vertex[slot as usize] = Some(slice);
        Ok(())
    }

    fn set_index_buffer(&mut self, slice: BufferSlice, format: IndexFormat) -> EngineResult<()> {
        self.current_index = Some((slice, format));
        Ok(())
    }

    fn draw(&mut self, args: DrawArgs) -> EngineResult<()> {
        let Some(pipeline_id) = self.current_pipeline else { return self.err("draw: no pipeline bound"); };
        let p = *self.pipelines.get(&pipeline_id).ok_or_else(|| EngineError::other("draw: invalid current pipeline"))?;

        let mut sets = [vk::DescriptorSet::null(); 4];
        let mut set_count = 0u32;
        for (i, bg_id) in self.current_bind_groups.iter().enumerate() {
            if let Some(bg_id) = bg_id {
                let bg = *self.bind_groups.get(bg_id).ok_or_else(|| EngineError::other("draw: invalid bind group"))?;
                sets[i] = bg.set;
                set_count = (i as u32) + 1;
            }
        }
        if set_count > 0 {
            self.recorded.push(RecordedCmd::BindDescriptorSets { layout: p.layout, first_set: 0, sets, set_count });
        }

        let mut bufs = [vk::Buffer::null(); 4];
        let mut offs = [0u64; 4];
        let mut count = 0u32;
        for (i, s) in self.current_vertex.iter().enumerate() {
            if let Some(s) = s {
                let b = *self.buffers.get(&s.buffer).ok_or_else(|| EngineError::other("draw: invalid vertex buffer"))?;
                bufs[i] = b.buffer;
                offs[i] = s.offset as u64;
                count = (i as u32) + 1;
            }
        }
        if count > 0 {
            self.recorded.push(RecordedCmd::BindVertexBuffer { first_binding: 0, buffers: bufs, offsets: offs, count });
        }

        self.recorded.push(RecordedCmd::Draw(args));
        Ok(())
    }

    fn draw_indexed(&mut self, args: DrawIndexedArgs) -> EngineResult<()> {
        let Some(pipeline_id) = self.current_pipeline else { return self.err("draw_indexed: no pipeline bound"); };
        let p = *self.pipelines.get(&pipeline_id).ok_or_else(|| EngineError::other("draw_indexed: invalid current pipeline"))?;

        let mut sets = [vk::DescriptorSet::null(); 4];
        let mut set_count = 0u32;
        for (i, bg_id) in self.current_bind_groups.iter().enumerate() {
            if let Some(bg_id) = bg_id {
                let bg = *self.bind_groups.get(bg_id).ok_or_else(|| EngineError::other("draw_indexed: invalid bind group"))?;
                sets[i] = bg.set;
                set_count = (i as u32) + 1;
            }
        }
        if set_count > 0 {
            self.recorded.push(RecordedCmd::BindDescriptorSets { layout: p.layout, first_set: 0, sets, set_count });
        }

        let mut bufs = [vk::Buffer::null(); 4];
        let mut offs = [0u64; 4];
        let mut count = 0u32;
        for (i, s) in self.current_vertex.iter().enumerate() {
            if let Some(s) = s {
                let b = *self.buffers.get(&s.buffer).ok_or_else(|| EngineError::other("draw_indexed: invalid vertex buffer"))?;
                bufs[i] = b.buffer;
                offs[i] = s.offset as u64;
                count = (i as u32) + 1;
            }
        }
        if count > 0 {
            self.recorded.push(RecordedCmd::BindVertexBuffer { first_binding: 0, buffers: bufs, offsets: offs, count });
        }

        let Some((idx_slice, fmt)) = self.current_index else { return self.err("draw_indexed: no index buffer bound"); };
        let ib = *self.buffers.get(&idx_slice.buffer).ok_or_else(|| EngineError::other("draw_indexed: invalid index buffer"))?;

        self.recorded.push(RecordedCmd::BindIndexBuffer {
            buffer: ib.buffer,
            offset: idx_slice.offset as vk::DeviceSize,
            index_type: Self::map_index_format(fmt),
        });

        self.recorded.push(RecordedCmd::DrawIndexed(args));
        Ok(())
    }
}
