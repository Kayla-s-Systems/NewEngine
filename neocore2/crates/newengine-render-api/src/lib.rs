#![forbid(unsafe_op_in_unsafe_fn)]

pub use newengine_ui_draw::{UiDrawCmd, UiDrawList, UiTexId, UiTexture, UiTextureDelta, UiTexturePatch, UiVertex};
pub mod reserved_textures {
    pub use newengine_ui_draw::reserved::*;
}
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

pub const RENDER_SERVICE_ID: &str = "render.api.v1";
pub const RENDER_SERVICE_METHOD_INVOKE_V1: &str = "invoke_json_v1";
pub const RENDER_SERVICE_METHOD_INFO_V1: &str = "info_json_v1";

pub type Color4 = [f32; 4];
pub type RenderWireResult<T> = Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeginFrameDesc {
    pub clear_color: Color4,
}

impl BeginFrameDesc {
    #[inline]
    pub const fn new(clear_color: Color4) -> Self {
        Self { clear_color }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderTargetDesc {
    pub extent: Extent2D,
    pub color: TextureFormat,
    pub depth: Option<TextureFormat>,
    pub label: Option<String>,
}

impl RenderTargetDesc {
    #[inline]
    pub fn new(extent: Extent2D, color: TextureFormat) -> Self {
        Self {
            extent,
            color,
            depth: None,
            label: None,
        }
    }

    #[inline]
    pub fn with_depth(mut self, depth: TextureFormat) -> Self {
        self.depth = Some(depth);
        self
    }

    #[inline]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeginRenderTargetDesc {
    pub target: RenderTargetId,
    pub clear_color: Option<Color4>,
    pub clear_depth: Option<f32>,
    pub clear_stencil: Option<u32>,
}

impl BeginRenderTargetDesc {
    #[inline]
    pub const fn new(target: RenderTargetId) -> Self {
        Self {
            target,
            clear_color: None,
            clear_depth: None,
            clear_stencil: None,
        }
    }

    #[inline]
    pub fn with_clear_color(mut self, color: Color4) -> Self {
        self.clear_color = Some(color);
        self
    }

    #[inline]
    pub fn with_clear_depth(mut self, depth: f32) -> Self {
        self.clear_depth = Some(depth);
        self
    }

    #[inline]
    pub fn with_clear_stencil(mut self, stencil: u32) -> Self {
        self.clear_stencil = Some(stencil);
        self
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferUsage {
    Vertex,
    Index,
    Uniform,
    Storage,
    Staging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryHint {
    GpuOnly,
    CpuToGpu,
    GpuToCpu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferDesc {
    pub label: Option<String>,
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
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureFormat {
    Rgba8Unorm,
    Bgra8Unorm,
    Rgba16Float,
    Depth24Stencil8,
    Depth32Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureUsage {
    Sampled,
    RenderTarget,
    DepthStencil,
    Storage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureDesc {
    pub label: Option<String>,
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
            mip_levels: NonZeroU32::new(1).expect("mip_levels must be non-zero"),
        }
    }

    #[inline]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[inline]
    pub fn with_mips(mut self, mip_levels: NonZeroU32) -> Self {
        self.mip_levels = mip_levels;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterMode {
    Nearest,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressMode {
    ClampToEdge,
    Repeat,
    MirroredRepeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplerDesc {
    pub label: Option<String>,
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
            address_u: AddressMode::ClampToEdge,
            address_v: AddressMode::ClampToEdge,
            address_w: AddressMode::ClampToEdge,
        }
    }
}

impl SamplerDesc {
    #[inline]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderDesc {
    pub label: Option<String>,
    pub stage: ShaderStage,
    pub entry: String,
    pub spirv: Vec<u32>,
}

impl ShaderDesc {
    #[inline]
    pub fn new(stage: ShaderStage, entry: impl Into<String>, spirv: Vec<u32>) -> Self {
        Self {
            label: None,
            stage,
            entry: entry.into(),
            spirv,
        }
    }

    #[inline]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveTopology {
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexFormat {
    U16,
    U32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VertexFormat {
    Float32x2,
    Float32x3,
    Float32x4,
    Unorm8x4,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineDesc {
    pub label: Option<String>,
    pub vs: ShaderId,
    pub fs: ShaderId,
    pub topology: PrimitiveTopology,
    pub vertex_layouts: Vec<VertexLayout>,
    pub bind_group_layouts: Vec<BindGroupLayoutId>,
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
            bind_group_layouts: Vec::new(),
            color_format,
            depth_format: None,
        }
    }

    #[inline]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
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
    pub fn with_bind_group_layouts(mut self, layouts: Vec<BindGroupLayoutId>) -> Self {
        self.bind_group_layouts = layouts;
        self
    }

    #[inline]
    pub fn push_bind_group_layout(mut self, layout: BindGroupLayoutId) -> Self {
        self.bind_group_layouts.push(layout);
        self
    }

    #[inline]
    pub fn with_depth(mut self, depth_format: TextureFormat) -> Self {
        self.depth_format = Some(depth_format);
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

macro_rules! define_id {
    ($name:ident, $vis_new:vis) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(NonZeroU32);

        impl $name {
            #[allow(dead_code)]
            #[inline]
            $vis_new fn new(v: u32) -> Self {
                Self(NonZeroU32::new(v).expect(concat!(stringify!($name), " must be non-zero")))
            }

            #[inline]
            pub fn get(self) -> u32 {
                self.0.get()
            }
        }
    };
}

define_id!(BufferId, pub);
define_id!(TextureId, pub);
define_id!(SamplerId, pub);
define_id!(ShaderId, pub);
define_id!(PipelineId, pub);
define_id!(BindGroupLayoutId, pub);
define_id!(BindGroupId, pub);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderTargetId(pub NonZeroU32);

impl RenderTargetId {
    #[inline]
    pub fn new(v: u32) -> Self {
        Self(NonZeroU32::new(v).expect("RenderTargetId must be non-zero"))
    }

    #[inline]
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingKind {
    Texture2D,
    Sampler,
    UniformBuffer,
    StorageBuffer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BufferBinding {
    pub buffer: BufferId,
    pub offset: u64,
    pub size: u64,
}

impl BufferBinding {
    #[inline]
    pub const fn new(buffer: BufferId, offset: u64, size: u64) -> Self {
        Self {
            buffer,
            offset,
            size,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindGroupLayoutDesc {
    pub label: Option<String>,
    pub bindings: Vec<BindingKind>,
}

impl BindGroupLayoutDesc {
    #[inline]
    pub fn new(bindings: Vec<BindingKind>) -> Self {
        Self {
            label: None,
            bindings,
        }
    }

    #[inline]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindGroupDesc {
    pub label: Option<String>,
    pub layout: BindGroupLayoutId,
    pub texture0: Option<TextureId>,
    pub sampler0: Option<SamplerId>,
    pub uniform0: Option<BufferBinding>,
    pub storage0: Option<BufferBinding>,
}

impl BindGroupDesc {
    #[inline]
    pub fn new(layout: BindGroupLayoutId) -> Self {
        Self {
            label: None,
            layout,
            texture0: None,
            sampler0: None,
            uniform0: None,
            storage0: None,
        }
    }

    #[inline]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[inline]
    pub fn with_texture0(mut self, tex: TextureId) -> Self {
        self.texture0 = Some(tex);
        self
    }

    #[inline]
    pub fn with_sampler0(mut self, s: SamplerId) -> Self {
        self.sampler0 = Some(s);
        self
    }

    #[inline]
    pub fn with_uniform0(mut self, b: BufferBinding) -> Self {
        self.uniform0 = Some(b);
        self
    }

    #[inline]
    pub fn with_storage0(mut self, b: BufferBinding) -> Self {
        self.storage0 = Some(b);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderBackendInfoV1 {
    pub backend_id: String,
    pub backend_name: String,
    pub backend_version: String,
    pub debug_text: String,
    pub clear_color: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderRequestV1 {
    BeginFrame(BeginFrameDesc),
    SetUiDrawList(UiDrawList),
    EndFrame,
    Resize { width: u32, height: u32 },
    CreateRenderTarget(RenderTargetDesc),
    DestroyRenderTarget { id: RenderTargetId },
    RenderTargetUiTexId { id: RenderTargetId },
    BeginRenderTarget(BeginRenderTargetDesc),
    EndRenderTarget,
    CreateBuffer(BufferDesc),
    DestroyBuffer { id: BufferId },
    WriteBuffer { id: BufferId, offset: u64, data: Vec<u8> },
    CreateTexture(TextureDesc),
    DestroyTexture { id: TextureId },
    CreateSampler(SamplerDesc),
    DestroySampler { id: SamplerId },
    CreateShader(ShaderDesc),
    DestroyShader { id: ShaderId },
    CreatePipeline(PipelineDesc),
    DestroyPipeline { id: PipelineId },
    CreateBindGroupLayout(BindGroupLayoutDesc),
    DestroyBindGroupLayout { id: BindGroupLayoutId },
    CreateBindGroup(BindGroupDesc),
    DestroyBindGroup { id: BindGroupId },
    SetViewport(Viewport),
    SetScissor(RectI32),
    SetPipeline { pipeline: PipelineId },
    SetBindGroup { index: u32, group: BindGroupId },
    SetVertexBuffer { slot: u32, slice: BufferSlice },
    SetIndexBuffer { slice: BufferSlice, format: IndexFormat },
    Draw(DrawArgs),
    DrawIndexed(DrawIndexedArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderResponseV1 {
    Unit,
    RenderTargetId(RenderTargetId),
    UiTexId(UiTexId),
    BufferId(BufferId),
    TextureId(TextureId),
    SamplerId(SamplerId),
    ShaderId(ShaderId),
    PipelineId(PipelineId),
    BindGroupLayoutId(BindGroupLayoutId),
    BindGroupId(BindGroupId),
}

#[inline]
pub fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}

#[inline]
pub fn decode_json<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
