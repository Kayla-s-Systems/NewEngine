#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    BindGroupId, BufferDesc, BufferId, BufferSlice, BufferUsage, DrawIndexedArgs, IndexFormat,
    MemoryHint, PipelineId, RenderApi, SamplerId, TextureId,
};
use newengine_core::EngineResult;
use newengine_math::collections::FxHashMap;
use newengine_math::Mat4;

use super::super::gpu::PrimitiveGpu;

/// Per-instance payload consumed by the runtime instanced lit/shadow shaders.
///
/// Layout is intentionally plain f32 arrays instead of glam/newengine math types:
/// this makes the buffer ABI obvious and stable across crate boundaries.
///
/// Vertex locations:
/// - 5..8   : model matrix columns
/// - 9..12  : pass MVP matrix columns
/// - 13     : base color
/// - 14     : UV transform
/// - 15     : material params
/// - 16     : emissive radiance + pad
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(in crate::render_controller) struct RenderInstanceRaw {
    model_cols: [[f32; 4]; 4],
    mvp_cols: [[f32; 4]; 4],
    base_color: [f32; 4],
    uv_transform: [f32; 4],
    material_params: [f32; 4],
    emissive_radiance: [f32; 4],
}

impl RenderInstanceRaw {
    #[inline]
    pub(in crate::render_controller) fn new(
        model: Mat4,
        mvp: Mat4,
        base_color: [f32; 4],
        uv_transform: [f32; 4],
        material_params: [f32; 4],
        emissive_radiance: [f32; 3],
    ) -> Self {
        Self {
            model_cols: mat4_cols(model),
            mvp_cols: mat4_cols(mvp),
            base_color,
            uv_transform,
            material_params,
            emissive_radiance: [
                emissive_radiance[0],
                emissive_radiance[1],
                emissive_radiance[2],
                0.0,
            ],
        }
    }

}

#[inline]
fn mat4_cols(m: Mat4) -> [[f32; 4]; 4] {
    let a = m.to_cols_array();
    [
        [a[0], a[1], a[2], a[3]],
        [a[4], a[5], a[6], a[7]],
        [a[8], a[9], a[10], a[11]],
        [a[12], a[13], a[14], a[15]],
    ]
}

#[inline]
pub(super) fn render_instances_as_bytes(instances: &[RenderInstanceRaw]) -> &[u8] {
    let byte_len = core::mem::size_of_val(instances);
    let ptr = instances.as_ptr().cast::<u8>();
    // SAFETY: `RenderInstanceRaw` is `repr(C)` and the slice is alive for the
    // returned byte view. The byte view is read-only and never outlives `instances`.
    unsafe { core::slice::from_raw_parts(ptr, byte_len) }
}

/// Frame-local, grow-only vertex buffer uploader for hardware instancing.
///
/// The buffer is intentionally reused across frames. It grows when capacity is
/// insufficient and keeps a cursor for sub-allocation. This avoids one buffer per
/// batch and keeps ownership inside the render controller instead of the Vulkan
/// plugin.
#[derive(Debug, Default)]
pub(in crate::render_controller) struct InstanceBufferUploader {
    buffer: Option<BufferId>,
    capacity_bytes: u64,
    cursor_bytes: u64,
}

impl InstanceBufferUploader {
    #[inline]
    pub(in crate::render_controller) fn begin_frame(&mut self) {
        self.cursor_bytes = 0;
    }

    pub(in crate::render_controller) fn upload(
        &mut self,
        r: &mut dyn RenderApi,
        instances: &[RenderInstanceRaw],
    ) -> EngineResult<BufferSlice> {
        let bytes = render_instances_as_bytes(instances);
        if bytes.is_empty() {
            return Err(newengine_core::EngineError::other(
                "instance upload requested with empty instance slice",
            ));
        }

        let required_end = self.cursor_bytes.saturating_add(bytes.len() as u64);
        self.ensure_capacity(r, required_end)?;

        let buffer = self.buffer.expect("instance buffer exists after ensure_capacity");
        let offset = self.cursor_bytes;
        r.write_buffer(buffer, offset, bytes)?;
        self.cursor_bytes = align_up(required_end, 256);
        Ok(BufferSlice::new(buffer, offset))
    }

    fn ensure_capacity(&mut self, r: &mut dyn RenderApi, required_bytes: u64) -> EngineResult<()> {
        if self.buffer.is_some() && self.capacity_bytes >= required_bytes {
            return Ok(());
        }

        let new_capacity = next_capacity(required_bytes.max(64 * 1024));
        // Do not destroy the previous instance buffer here. Commands already
        // recorded earlier in this frame may still reference it, and backends
        // may have multiple frames in flight. The uploader is grow-only for now;
        // a future resource lifetime queue can retire old buffers after fences.
        self.buffer = None;

        let buffer = r.create_buffer(
            BufferDesc::new(new_capacity, BufferUsage::Vertex, MemoryHint::CpuToGpu)
                .with_label(format!("runtime_instance_buffer:{}kb", new_capacity / 1024)),
        )?;
        self.buffer = Some(buffer);
        self.capacity_bytes = new_capacity;
        self.cursor_bytes = 0;
        Ok(())
    }
}

#[inline]
fn align_up(v: u64, alignment: u64) -> u64 {
    debug_assert!(alignment.is_power_of_two());
    (v + alignment - 1) & !(alignment - 1)
}

#[inline]
fn next_capacity(required: u64) -> u64 {
    required.next_power_of_two().max(64 * 1024)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct InstanceBatchKey {
    pub(super) pipeline: u32,
    pub(super) bind_group: u32,
    pub(super) vertex_buffer: u32,
    pub(super) index_buffer: u32,
    pub(super) index_format: u8,
    pub(super) first_index: u32,
    pub(super) vertex_offset: i32,
    pub(super) base_texture: u32,
    pub(super) normal_texture: u32,
    pub(super) roughness_texture: u32,
    pub(super) shadow_texture: u32,
    pub(super) sampler: u32,
    pub(super) mesh_key: u64,
}

impl InstanceBatchKey {
    #[inline]
    pub(in crate::render_controller) fn new(
        pipeline: PipelineId,
        bind_group: BindGroupId,
        gpu: PrimitiveGpu,
        base_texture: TextureId,
        normal_texture: TextureId,
        roughness_texture: TextureId,
        shadow_texture: TextureId,
        sampler: SamplerId,
        mesh_key: u64,
    ) -> Self {
        Self {
            pipeline: pipeline.get(),
            bind_group: bind_group.get(),
            vertex_buffer: gpu.vb.get(),
            index_buffer: gpu.ib.get(),
            index_format: 32,
            first_index: 0,
            vertex_offset: 0,
            base_texture: base_texture.get(),
            normal_texture: normal_texture.get(),
            roughness_texture: roughness_texture.get(),
            shadow_texture: shadow_texture.get(),
            sampler: sampler.get(),
            mesh_key,
        }
    }
}

pub(super) struct InstanceBatch {
    pub(super) key: InstanceBatchKey,
    pub(super) pipeline: PipelineId,
    pub(super) bind_group: BindGroupId,
    pub(super) gpu: PrimitiveGpu,
    pub(super) instances: Vec<RenderInstanceRaw>,
}

impl InstanceBatch {
    #[inline]
    fn new(
        key: InstanceBatchKey,
        pipeline: PipelineId,
        bind_group: BindGroupId,
        gpu: PrimitiveGpu,
    ) -> Self {
        Self {
            key,
            pipeline,
            bind_group,
            gpu,
            instances: Vec::new(),
        }
    }
}

#[derive(Default)]
pub(super) struct InstanceBatchSet {
    batches: FxHashMap<InstanceBatchKey, InstanceBatch>,
}

impl InstanceBatchSet {
    #[inline]
    pub(super) fn push(
        &mut self,
        key: InstanceBatchKey,
        pipeline: PipelineId,
        bind_group: BindGroupId,
        gpu: PrimitiveGpu,
        instance: RenderInstanceRaw,
    ) {
        self.batches
            .entry(key)
            .or_insert_with(|| InstanceBatch::new(key, pipeline, bind_group, gpu))
            .instances
            .push(instance);
    }

    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }

    pub(super) fn into_sorted_batches(self) -> Vec<InstanceBatch> {
        let mut batches = self.batches.into_values().collect::<Vec<_>>();
        batches.sort_by_key(|b| b.key);
        batches
    }
}

#[derive(Default)]
pub(super) struct InstancedReplayState {
    pipeline: Option<PipelineId>,
    bind_group0: Option<BindGroupId>,
    vertex0: Option<BufferSliceKey>,
    vertex1: Option<BufferSliceKey>,
    index: Option<IndexBufferKey>,
}

impl InstancedReplayState {
    #[inline]
    pub(super) fn set_pipeline(
        &mut self,
        r: &mut dyn RenderApi,
        pipeline: PipelineId,
    ) -> EngineResult<()> {
        if self.pipeline != Some(pipeline) {
            r.set_pipeline(pipeline)?;
            self.pipeline = Some(pipeline);
            self.bind_group0 = None;
        }
        Ok(())
    }

    #[inline]
    pub(super) fn set_bind_group0(
        &mut self,
        r: &mut dyn RenderApi,
        bind_group: BindGroupId,
    ) -> EngineResult<()> {
        if self.bind_group0 != Some(bind_group) {
            r.set_bind_group(0, bind_group)?;
            self.bind_group0 = Some(bind_group);
        }
        Ok(())
    }

    #[inline]
    pub(super) fn set_vertex_buffer(
        &mut self,
        r: &mut dyn RenderApi,
        slot: u32,
        slice: BufferSlice,
    ) -> EngineResult<()> {
        let key = BufferSliceKey::from(slice);
        let target = match slot {
            0 => &mut self.vertex0,
            1 => &mut self.vertex1,
            _ => {
                return Err(newengine_core::EngineError::other(format!(
                    "instanced replay supports vertex slots 0/1 only, got {slot}"
                )));
            }
        };
        if *target != Some(key) {
            r.set_vertex_buffer(slot, slice)?;
            *target = Some(key);
        }
        Ok(())
    }

    #[inline]
    pub(super) fn set_index_buffer(
        &mut self,
        r: &mut dyn RenderApi,
        slice: BufferSlice,
        format: IndexFormat,
    ) -> EngineResult<()> {
        let key = IndexBufferKey::new(slice, format);
        if self.index != Some(key) {
            r.set_index_buffer(slice, format)?;
            self.index = Some(key);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BufferSliceKey {
    buffer: u32,
    offset: u64,
}

impl From<BufferSlice> for BufferSliceKey {
    #[inline]
    fn from(value: BufferSlice) -> Self {
        Self {
            buffer: value.buffer.get(),
            offset: value.offset,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IndexBufferKey {
    slice: BufferSliceKey,
    format: u8,
}

impl IndexBufferKey {
    #[inline]
    fn new(slice: BufferSlice, format: IndexFormat) -> Self {
        Self {
            slice: BufferSliceKey::from(slice),
            format: match format {
                IndexFormat::U16 => 16,
                IndexFormat::U32 => 32,
            },
        }
    }
}

#[inline]
pub(super) fn draw_indexed_instanced_args(index_count: u32, instance_count: u32) -> DrawIndexedArgs {
    DrawIndexedArgs {
        index_count,
        instance_count,
        first_index: 0,
        vertex_offset: 0,
        first_instance: 0,
    }
}