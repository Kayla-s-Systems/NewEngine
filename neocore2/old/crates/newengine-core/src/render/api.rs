use crate::error::{EngineError, EngineResult};
use crate::module::{ApiProvide, ApiVersion};

use parking_lot::{Mutex, MutexGuard};
use std::num::NonZeroU32;
use std::sync::Arc;

pub const RENDER_API_ID: &str = "render.api";
pub const RENDER_API_VERSION: ApiVersion = ApiVersion::new(0, 2, 0);
pub const RENDER_API_PROVIDE: ApiProvide = ApiProvide::new(RENDER_API_ID, RENDER_API_VERSION);

pub type Color4 = [f32; 4];

#[derive(Debug, Clone, Copy)]
pub struct BeginFrameDesc {
    pub clear_color: Color4,
}

impl BeginFrameDesc {
    #[inline]
    pub const fn new(clear_color: Color4) -> Self {
        Self { clear_color }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Extent2D {
    pub width: u32,
    pub height: u32,
}

impl Extent2D {
    #[inline]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferUsage {
    Vertex,
    Index,
    Uniform,
    Storage,
    Staging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryHint {
    GpuOnly,
    CpuToGpu,
    GpuToCpu,
}

#[derive(Debug, Clone)]
pub struct BufferDesc {
    pub label: Option<&'static str>,
    pub size: u64,
    pub usage: BufferUsage,
    pub memory: MemoryHint,
}

impl BufferDesc {
    #[inline]
    pub fn new(size: u64, usage: BufferUsage, memory: MemoryHint) -> Self {
        Self {
            label: None,
            size,
            usage,
            memory,
        }
    }

    #[inline]
    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8Unorm,
    Bgra8Unorm,
    Rgba16Float,
    Depth24Stencil8,
    Depth32Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureUsage {
    Sampled,
    RenderTarget,
    DepthStencil,
    Storage,
}

#[derive(Debug, Clone)]
pub struct TextureDesc {
    pub label: Option<&'static str>,
    pub extent: Extent2D,
    pub format: TextureFormat,
    pub usage: TextureUsage,
    pub mip_levels: NonZeroU32,
}

impl TextureDesc {
    #[inline]
    pub fn new(extent: Extent2D, format: TextureFormat, usage: TextureUsage) -> Self {
        Self {
            label: None,
            extent,
            format,
            usage,
            mip_levels: NonZeroU32::new(1).unwrap(),
        }
    }

    #[inline]
    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    #[inline]
    pub fn with_mips(mut self, mip_levels: NonZeroU32) -> Self {
        self.mip_levels = mip_levels;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Nearest,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressMode {
    ClampToEdge,
    Repeat,
    MirroredRepeat,
}

#[derive(Debug, Clone)]
pub struct SamplerDesc {
    pub label: Option<&'static str>,
    pub min_filter: FilterMode,
    pub mag_filter: FilterMode,
    pub mip_filter: FilterMode,
    pub address_u: AddressMode,
    pub address_v: AddressMode,
    pub address_w: AddressMode,
}

impl Default for SamplerDesc {
    #[inline]
    fn default() -> Self {
        Self {
            label: None,
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            mip_filter: FilterMode::Linear,
            address_u: AddressMode::Repeat,
            address_v: AddressMode::Repeat,
            address_w: AddressMode::Repeat,
        }
    }
}

impl SamplerDesc {
    #[inline]
    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

#[derive(Debug, Clone)]
pub struct ShaderDesc {
    pub label: Option<&'static str>,
    pub stage: ShaderStage,
    pub entry: &'static str,
    pub spirv: Vec<u32>,
}

impl ShaderDesc {
    #[inline]
    pub fn new(stage: ShaderStage, entry: &'static str, spirv: Vec<u32>) -> Self {
        Self {
            label: None,
            stage,
            entry,
            spirv,
        }
    }

    #[inline]
    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveTopology {
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexFormat {
    U16,
    U32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexFormat {
    Float32x2,
    Float32x3,
    Float32x4,
    Unorm8x4,
}

#[derive(Debug, Clone, Copy)]
pub struct VertexAttribute {
    pub location: u32,
    pub offset: u32,
    pub format: VertexFormat,
}

impl VertexAttribute {
    #[inline]
    pub const fn new(location: u32, offset: u32, format: VertexFormat) -> Self {
        Self {
            location,
            offset,
            format,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VertexLayout {
    pub stride: u32,
    pub attributes: Vec<VertexAttribute>,
}

impl VertexLayout {
    #[inline]
    pub fn new(stride: u32, attributes: Vec<VertexAttribute>) -> Self {
        Self { stride, attributes }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineDesc {
    pub label: Option<&'static str>,
    pub vs: ShaderId,
    pub fs: ShaderId,
    pub topology: PrimitiveTopology,
    pub vertex_layouts: Vec<VertexLayout>,
    pub color_format: TextureFormat,
    pub depth_format: Option<TextureFormat>,
}

impl PipelineDesc {
    #[inline]
    pub fn new(vs: ShaderId, fs: ShaderId, color_format: TextureFormat) -> Self {
        Self {
            label: None,
            vs,
            fs,
            topology: PrimitiveTopology::TriangleList,
            vertex_layouts: Vec::new(),
            color_format,
            depth_format: None,
        }
    }

    #[inline]
    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    #[inline]
    pub fn with_topology(mut self, topology: PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    #[inline]
    pub fn with_vertex_layouts(mut self, layouts: Vec<VertexLayout>) -> Self {
        self.vertex_layouts = layouts;
        self
    }

    #[inline]
    pub fn with_depth(mut self, depth_format: TextureFormat) -> Self {
        self.depth_format = Some(depth_format);
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

impl Viewport {
    #[inline]
    pub fn full(extent: Extent2D) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: extent.width as f32,
            h: extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RectI32 {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl RectI32 {
    #[inline]
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(NonZeroU32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(NonZeroU32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SamplerId(NonZeroU32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderId(NonZeroU32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineId(NonZeroU32);

impl BufferId {
    #[inline]
    pub(crate) fn new(v: u32) -> Self {
        Self(NonZeroU32::new(v).expect("BufferId must be non-zero"))
    }
}

impl TextureId {
    #[inline]
    pub(crate) fn new(v: u32) -> Self {
        Self(NonZeroU32::new(v).expect("TextureId must be non-zero"))
    }
}

impl SamplerId {
    #[inline]
    pub(crate) fn new(v: u32) -> Self {
        Self(NonZeroU32::new(v).expect("SamplerId must be non-zero"))
    }
}

impl ShaderId {
    #[inline]
    pub(crate) fn new(v: u32) -> Self {
        Self(NonZeroU32::new(v).expect("ShaderId must be non-zero"))
    }
}

impl PipelineId {
    #[inline]
    pub(crate) fn new(v: u32) -> Self {
        Self(NonZeroU32::new(v).expect("PipelineId must be non-zero"))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BufferSlice {
    pub buffer: BufferId,
    pub offset: u64,
}

impl BufferSlice {
    #[inline]
    pub const fn new(buffer: BufferId, offset: u64) -> Self {
        Self { buffer, offset }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DrawArgs {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

impl DrawArgs {
    #[inline]
    pub const fn new(vertex_count: u32) -> Self {
        Self {
            vertex_count,
            instance_count: 1,
            first_vertex: 0,
            first_instance: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DrawIndexedArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}

impl DrawIndexedArgs {
    #[inline]
    pub const fn new(index_count: u32) -> Self {
        Self {
            index_count,
            instance_count: 1,
            first_index: 0,
            vertex_offset: 0,
            first_instance: 0,
        }
    }
}

/// Render backend contract.
///
/// Object-safe, used through Arc<Mutex<...>>. Commands are recorded between begin_frame/end_frame.
pub trait RenderApi: Send {
    fn begin_frame(&mut self, desc: BeginFrameDesc) -> EngineResult<()>;
    fn end_frame(&mut self) -> EngineResult<()>;
    fn resize(&mut self, width: u32, height: u32) -> EngineResult<()>;

    fn create_buffer(&mut self, desc: BufferDesc) -> EngineResult<BufferId>;
    fn destroy_buffer(&mut self, id: BufferId);

    fn write_buffer(&mut self, id: BufferId, offset: u64, data: &[u8]) -> EngineResult<()>;

    fn create_texture(&mut self, desc: TextureDesc) -> EngineResult<TextureId>;
    fn destroy_texture(&mut self, id: TextureId);

    fn create_sampler(&mut self, desc: SamplerDesc) -> EngineResult<SamplerId>;
    fn destroy_sampler(&mut self, id: SamplerId);

    fn create_shader(&mut self, desc: ShaderDesc) -> EngineResult<ShaderId>;
    fn destroy_shader(&mut self, id: ShaderId);

    fn create_pipeline(&mut self, desc: PipelineDesc) -> EngineResult<PipelineId>;
    fn destroy_pipeline(&mut self, id: PipelineId);

    fn set_viewport(&mut self, vp: Viewport) -> EngineResult<()>;
    fn set_scissor(&mut self, rect: RectI32) -> EngineResult<()>;

    fn set_pipeline(&mut self, pipeline: PipelineId) -> EngineResult<()>;
    fn set_vertex_buffer(&mut self, slot: u32, slice: BufferSlice) -> EngineResult<()>;
    fn set_index_buffer(&mut self, slice: BufferSlice, format: IndexFormat) -> EngineResult<()>;

    fn draw(&mut self, args: DrawArgs) -> EngineResult<()>;
    fn draw_indexed(&mut self, args: DrawIndexedArgs) -> EngineResult<()>;
}

#[derive(Clone)]
pub struct RenderApiRef(Arc<Mutex<Box<dyn RenderApi + Send + 'static>>>);

impl RenderApiRef {
    #[inline]
    pub fn new(api: impl RenderApi + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Box::new(api))))
    }

    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, Box<dyn RenderApi + Send + 'static>> {
        self.0.lock()
    }
}

#[inline]
pub fn require_render_api<'a, E: Send + 'static>(
    ctx: &'a crate::module::ModuleCtx<'_, E>,
) -> EngineResult<&'a RenderApiRef> {
    ctx.api_required::<RenderApiRef>(RENDER_API_ID).map_err(|_| {
        EngineError::other("Render API is not available (missing render backend module?)")
    })
}