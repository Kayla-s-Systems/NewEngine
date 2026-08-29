#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    BindGroupId, BufferSlice, DrawIndexedArgs, IndexFormat, PipelineId, RenderApi,
};
use newengine_core::EngineResult;

/// CPU-side draw bucketing for the current immediate RenderApi path.
///
/// This is intentionally not real GPU instancing yet: the lit shaders still use a
/// per-draw UBO for transforms/material parameters. The purpose of this layer is
/// to establish the stable packet -> bucket -> replay architecture and eliminate
/// redundant state commands while the renderer backend evolves toward instance
/// buffers / multi-draw.
#[derive(Debug, Default)]
pub(super) struct BucketedIndexedDrawStream {
    packets: Vec<IndexedDrawPacket>,
}

impl BucketedIndexedDrawStream {
    #[inline]
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            packets: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub(super) fn push(&mut self, packet: IndexedDrawPacket) {
        if packet.args.index_count == 0 || packet.args.instance_count == 0 {
            return;
        }
        self.packets.push(packet);
    }

    pub(super) fn emit_sorted(mut self, r: &mut dyn RenderApi) -> EngineResult<()> {
        self.packets.sort_by_key(IndexedDrawPacket::bucket_key);
        let mut state = DrawStateCache::default();

        for packet in self.packets {
            state.set_pipeline(r, packet.pipeline)?;
            state.set_vertex_buffer(r, 0, packet.vertex)?;
            state.set_index_buffer(r, packet.index, packet.index_format)?;
            state.set_bind_group(r, 0, packet.bind_group)?;
            r.draw_indexed(packet.args)?;
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct IndexedDrawPacket {
    pub(super) pipeline: PipelineId,
    pub(super) bind_group: BindGroupId,
    pub(super) vertex: BufferSlice,
    pub(super) index: BufferSlice,
    pub(super) index_format: IndexFormat,
    pub(super) args: DrawIndexedArgs,
}

impl IndexedDrawPacket {
    #[inline]
    fn bucket_key(&self) -> IndexedDrawBucketKey {
        IndexedDrawBucketKey {
            pipeline: self.pipeline.get(),
            vertex_buffer: self.vertex.buffer.get(),
            vertex_offset: self.vertex.offset,
            index_buffer: self.index.buffer.get(),
            index_offset: self.index.offset,
            index_format: index_format_key(self.index_format),
            bind_group: self.bind_group.get(),
            first_index: self.args.first_index,
            vertex_offset_arg: self.args.vertex_offset,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct IndexedDrawBucketKey {
    pipeline: u32,
    vertex_buffer: u32,
    vertex_offset: u64,
    index_buffer: u32,
    index_offset: u64,
    index_format: u8,
    bind_group: u32,
    first_index: u32,
    vertex_offset_arg: i32,
}

#[derive(Default)]
struct DrawStateCache {
    pipeline: Option<PipelineId>,
    vertex_slot0: Option<BufferSliceKey>,
    index: Option<IndexBufferKey>,
    bind_group0: Option<BindGroupId>,
}

impl DrawStateCache {
    #[inline]
    fn set_pipeline(&mut self, r: &mut dyn RenderApi, pipeline: PipelineId) -> EngineResult<bool> {
        if self.pipeline == Some(pipeline) {
            return Ok(false);
        }
        r.set_pipeline(pipeline)?;
        self.pipeline = Some(pipeline);
        Ok(true)
    }

    #[inline]
    fn set_vertex_buffer(
        &mut self,
        r: &mut dyn RenderApi,
        slot: u32,
        slice: BufferSlice,
    ) -> EngineResult<bool> {
        debug_assert_eq!(
            slot, 0,
            "bucketed lit draw stream currently owns vertex slot 0 only"
        );
        let key = BufferSliceKey::from(slice);
        if self.vertex_slot0 == Some(key) {
            return Ok(false);
        }
        r.set_vertex_buffer(slot, slice)?;
        self.vertex_slot0 = Some(key);
        Ok(true)
    }

    #[inline]
    fn set_index_buffer(
        &mut self,
        r: &mut dyn RenderApi,
        slice: BufferSlice,
        format: IndexFormat,
    ) -> EngineResult<bool> {
        let key = IndexBufferKey::new(slice, format);
        if self.index == Some(key) {
            return Ok(false);
        }
        r.set_index_buffer(slice, format)?;
        self.index = Some(key);
        Ok(true)
    }

    #[inline]
    fn set_bind_group(
        &mut self,
        r: &mut dyn RenderApi,
        slot: u32,
        bind_group: BindGroupId,
    ) -> EngineResult<bool> {
        debug_assert_eq!(
            slot, 0,
            "bucketed lit draw stream currently owns bind group slot 0 only"
        );
        if self.bind_group0 == Some(bind_group) {
            return Ok(false);
        }
        r.set_bind_group(slot, bind_group)?;
        self.bind_group0 = Some(bind_group);
        Ok(true)
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
            format: index_format_key(format),
        }
    }
}

#[inline]
const fn index_format_key(format: IndexFormat) -> u8 {
    match format {
        IndexFormat::U16 => 16,
        IndexFormat::U32 => 32,
    }
}
