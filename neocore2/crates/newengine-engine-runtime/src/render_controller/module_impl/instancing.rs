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
/// - 16     : emissive radiance + alpha cutoff / opaque diagnostic token
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
        alpha_cutoff: f32,
        diagnostic_instance_id: u64,
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
                diagnostic_alpha_lane(alpha_cutoff, diagnostic_instance_id),
            ],
        }
    }

    #[inline]
    pub(in crate::render_controller) fn with_foliage_wind(
        mut self,
        enabled: bool,
        direction: [f32; 3],
        strength: f32,
    ) -> Self {
        let horizontal_len = (direction[0] * direction[0] + direction[2] * direction[2]).sqrt();
        let (dir_x, dir_z) = if horizontal_len > 1.0e-5 {
            (direction[0] / horizontal_len, direction[2] / horizontal_len)
        } else {
            (1.0, 0.0)
        };
        self.emissive_radiance[0] = dir_x;
        self.emissive_radiance[1] = dir_z;
        self.emissive_radiance[2] = if enabled { strength.max(0.0) } else { 0.0 };
        if self.emissive_radiance[3] > 0.0 {
            self.emissive_radiance[3] += 10.0;
        } else {
            self.emissive_radiance[3] -= 20_000_000.0;
        }
        self
    }
}

#[inline]
pub(in crate::render_controller) fn diagnostic_instance_token(diagnostic_instance_id: u64) -> u32 {
    let folded = (diagnostic_instance_id as u32) ^ ((diagnostic_instance_id >> 32) as u32);
    let hashed = folded.wrapping_mul(0x9E37_79B1) ^ folded.rotate_left(13);
    hashed & 0x00FF_FFFF
}

fn diagnostic_alpha_lane(alpha_cutoff: f32, diagnostic_instance_id: u64) -> f32 {
    let alpha_cutoff = alpha_cutoff.max(0.0);
    if alpha_cutoff > 0.0 {
        return alpha_cutoff;
    }

    // Opaque materials do not consume the alpha-cutoff lane. Reuse that one float
    // as a negative 24-bit per-instance token for receiver diagnostics without
    // changing the long-lived 192-byte instance-buffer ABI. Integers up to 2^24
    // are exactly representable in f32; the negative sign keeps alpha testing off.
    -((diagnostic_instance_token(diagnostic_instance_id) as f32) + 1.0)
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
/// batch and keeps ownership inside the render controller instead of the backend
/// plugin.
#[derive(Debug)]
pub(in crate::render_controller) struct PackedInstanceUpload {
    pub(in crate::render_controller) slices: Vec<BufferSlice>,
    pub(in crate::render_controller) instance_count: usize,
    pub(in crate::render_controller) bytes_written: u64,
}

const INSTANCE_UPLOAD_FRAME_SLOTS: usize = 4;

#[derive(Debug, Default)]
struct InstanceUploadFrameSlot {
    buffer: Option<BufferId>,
    capacity_bytes: u64,
    cursor_bytes: u64,
}

#[derive(Debug)]
pub(in crate::render_controller) struct InstanceBufferUploader {
    /// One persistent upload arena per CPU/GPU frame slot. A single mapped buffer
    /// cannot be rewound while an older submitted frame may still read it: Vulkan
    /// vertex bindings reference memory, they do not snapshot bytes at draw-record time.
    /// Four engine slots exceed the current first-party backend depth (Vulkan = 2).
    frame_slots: [InstanceUploadFrameSlot; INSTANCE_UPLOAD_FRAME_SLOTS],
    active_slot: usize,
    /// Reused CPU staging storage. After the first peak frame, packed instance
    /// uploads no longer allocate a flattening vector for every render pass.
    staging_instances: Vec<RenderInstanceRaw>,
}

impl Default for InstanceBufferUploader {
    fn default() -> Self {
        Self {
            frame_slots: std::array::from_fn(|_| InstanceUploadFrameSlot::default()),
            active_slot: 0,
            staging_instances: Vec::new(),
        }
    }
}

impl InstanceBufferUploader {
    #[inline]
    pub(in crate::render_controller) fn begin_frame(&mut self, frame_index: u64) {
        self.active_slot = frame_index as usize % INSTANCE_UPLOAD_FRAME_SLOTS;
        self.frame_slots[self.active_slot].cursor_bytes = 0;
    }

    /// Uploads all sorted instance batches with one backend buffer write.
    ///
    /// Individual batches still receive distinct `BufferSlice` offsets, but the
    /// CPU-to-GPU transfer is coalesced. This avoids one service/backend write
    /// command per material/mesh batch while preserving draw ordering.
    pub(super) fn upload_batches(
        &mut self,
        r: &mut dyn RenderApi,
        batches: &[InstanceBatch],
    ) -> EngineResult<PackedInstanceUpload> {
        if batches.is_empty() {
            return Err(newengine_core::EngineError::other(
                "packed instance upload requested with no batches",
            ));
        }

        let instance_count = batches
            .iter()
            .map(|batch| batch.instances.len())
            .sum::<usize>();
        if instance_count == 0 {
            return Err(newengine_core::EngineError::other(
                "packed instance upload requested with no instances",
            ));
        }

        let stride = core::mem::size_of::<RenderInstanceRaw>() as u64;
        let byte_len = (instance_count as u64).saturating_mul(stride);
        let cursor_bytes = self.frame_slots[self.active_slot].cursor_bytes;
        let required_end = cursor_bytes.saturating_add(byte_len);
        self.ensure_capacity(r, required_end)?;

        let buffer = self.frame_slots[self.active_slot]
            .buffer
            .expect("instance buffer exists after ensure_capacity");
        let base_offset = cursor_bytes;
        self.staging_instances.clear();
        self.staging_instances
            .reserve(instance_count.saturating_sub(self.staging_instances.capacity()));
        let mut slices = Vec::with_capacity(batches.len());
        let mut running_instances = 0u64;

        for batch in batches {
            let offset = base_offset.saturating_add(running_instances.saturating_mul(stride));
            slices.push(BufferSlice::new(buffer, offset));
            self.staging_instances.extend_from_slice(&batch.instances);
            running_instances = running_instances.saturating_add(batch.instances.len() as u64);
        }

        r.write_buffer(
            buffer,
            base_offset,
            render_instances_as_bytes(&self.staging_instances),
        )?;
        self.frame_slots[self.active_slot].cursor_bytes = align_up(required_end, 256);

        Ok(PackedInstanceUpload {
            slices,
            instance_count,
            bytes_written: byte_len,
        })
    }

    fn ensure_capacity(&mut self, r: &mut dyn RenderApi, required_bytes: u64) -> EngineResult<()> {
        let slot = &self.frame_slots[self.active_slot];
        if slot.buffer.is_some() && slot.capacity_bytes >= required_bytes {
            return Ok(());
        }

        let new_capacity = next_capacity(required_bytes.max(64 * 1024));
        // Do not destroy the previous instance buffer here. Commands recorded in
        // this slot's previous use can still exist until the backend fence retires it.
        // The slot is grow-only until buffer-generation retirement is exposed via RenderApi.
        let buffer = r.create_buffer(
            BufferDesc::new(new_capacity, BufferUsage::Vertex, MemoryHint::CpuToGpu).with_label(
                format!(
                    "runtime_instance_buffer:slot{}:{}kb",
                    self.active_slot,
                    new_capacity / 1024
                ),
            ),
        )?;
        let slot = &mut self.frame_slots[self.active_slot];
        slot.buffer = Some(buffer);
        slot.capacity_bytes = new_capacity;
        slot.cursor_bytes = 0;
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

    #[inline]
    pub(super) fn batch_count(&self) -> usize {
        self.batches.len()
    }

    #[inline]
    pub(super) fn instance_count(&self) -> usize {
        self.batches
            .values()
            .map(|batch| batch.instances.len())
            .sum()
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
pub(super) fn draw_indexed_instanced_args(
    index_count: u32,
    instance_count: u32,
) -> DrawIndexedArgs {
    DrawIndexedArgs {
        index_count,
        instance_count,
        first_index: 0,
        vertex_offset: 0,
        first_instance: 0,
    }
}

#[cfg(test)]
mod instance_payload_tests {
    use super::*;
    use newengine_material_domain_api::LIT_INSTANCE_VERTEX_STRIDE;
    use newengine_math::Vec3;

    #[test]
    fn render_instance_raw_matches_shader_abi_stride() {
        assert_eq!(
            core::mem::size_of::<RenderInstanceRaw>(),
            LIT_INSTANCE_VERTEX_STRIDE as usize,
            "RenderInstanceRaw must stay byte-identical to locations 5..16 in the instanced shader",
        );
    }

    #[test]
    fn frame_upload_arena_rotates_without_rewinding_adjacent_gpu_frames() {
        let mut uploader = InstanceBufferUploader::default();
        uploader.begin_frame(1);
        let first = uploader.active_slot;
        uploader.frame_slots[first].cursor_bytes = 4096;

        uploader.begin_frame(2);
        let second = uploader.active_slot;
        assert_ne!(first, second);
        assert_eq!(uploader.frame_slots[first].cursor_bytes, 4096);
        assert_eq!(uploader.frame_slots[second].cursor_bytes, 0);

        uploader.begin_frame(1 + INSTANCE_UPLOAD_FRAME_SLOTS as u64);
        assert_eq!(uploader.active_slot, first);
        assert_eq!(uploader.frame_slots[first].cursor_bytes, 0);
    }

    #[test]
    fn material_payload_is_invariant_across_instance_transforms() {
        let base_color = [0.82, 0.73, 0.61, 1.0];
        let uv_transform = [1.0, 1.0, 0.125, -0.25];
        let material_params = [0.0, 0.68, 0.0, 1.0];
        let emissive = [0.0, 0.0, 0.0];

        let a = RenderInstanceRaw::new(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            base_color,
            uv_transform,
            material_params,
            emissive,
            0.0,
            0x1020_3040,
        );
        let translated = Mat4::from_translation(Vec3::new(3.0, 5.0, -2.0));
        let b = RenderInstanceRaw::new(
            translated,
            translated,
            base_color,
            uv_transform,
            material_params,
            emissive,
            0.0,
            0x1020_3040,
        );

        assert_eq!(a.base_color, b.base_color);
        assert_eq!(a.uv_transform, b.uv_transform);
        assert_eq!(a.material_params, b.material_params);
        assert_eq!(a.emissive_radiance, b.emissive_radiance);
        assert_eq!(a.emissive_radiance[3], b.emissive_radiance[3]);
        assert_ne!(a.model_cols, b.model_cols);
        assert_ne!(a.mvp_cols, b.mvp_cols);
    }

    #[test]
    fn opaque_diagnostic_instance_token_preserves_instance_stride_and_alpha_semantics() {
        let opaque = RenderInstanceRaw::new(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            [1.0; 4],
            [1.0, 1.0, 0.0, 0.0],
            [0.0, 0.68, 0.0, 1.0],
            [0.0; 3],
            0.0,
            0x0123_4567_89AB_CDEF,
        );
        assert!(opaque.emissive_radiance[3] < 0.0);

        let cutout = RenderInstanceRaw::new(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            [1.0; 4],
            [1.0, 1.0, 0.0, 0.0],
            [0.0, 0.68, 0.0, 1.0],
            [0.0; 3],
            0.42,
            0x0123_4567_89AB_CDEF,
        );
        assert!((cutout.emissive_radiance[3] - 0.42).abs() < 1.0e-6);
        assert_eq!(core::mem::size_of::<RenderInstanceRaw>(), 192);
    }

    #[test]
    fn packed_instance_bytes_are_dense_and_stride_aligned() {
        let instances = [
            RenderInstanceRaw::new(
                Mat4::IDENTITY,
                Mat4::IDENTITY,
                [1.0, 1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0, 0.0],
                [0.0, 0.68, 0.0, 1.0],
                [0.0, 0.0, 0.0],
                0.0,
                1,
            ),
            RenderInstanceRaw::new(
                Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
                Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
                [1.0, 1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0, 0.0],
                [0.0, 0.68, 0.0, 1.0],
                [0.0, 0.0, 0.0],
                0.0,
                2,
            ),
        ];
        let bytes = render_instances_as_bytes(&instances);
        assert_eq!(
            bytes.len(),
            instances.len() * LIT_INSTANCE_VERTEX_STRIDE as usize,
        );
    }
}
