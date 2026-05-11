use crate::{BindGroupLayoutId, ShaderId, TextureFormat};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveTopology {
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RasterCullMode {
    None,
    Front,
    Back,
}

impl Default for RasterCullMode {
    #[inline]
    fn default() -> Self {
        Self::Back
    }
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
    #[serde(default)]
    pub cull_mode: RasterCullMode,
    #[serde(default)]
    pub cache_key: Option<String>,
    #[serde(default)]
    pub warmup: bool,
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
            cull_mode: RasterCullMode::Back,
            cache_key: None,
            warmup: false,
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

    #[inline]
    pub fn with_cull_mode(mut self, cull_mode: RasterCullMode) -> Self {
        self.cull_mode = cull_mode;
        self
    }

    #[inline]
    pub fn with_cache_key(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.cache_key = if key.trim().is_empty() { None } else { Some(key) };
        self
    }

    #[inline]
    pub fn as_warmup(mut self) -> Self {
        self.warmup = true;
        self
    }
}
