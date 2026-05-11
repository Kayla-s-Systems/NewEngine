use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderGraphResourceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphResourceLifetime {
    Persistent,
    TransientFrame,
    Frames(u32),
    External,
}

impl Default for RenderGraphResourceLifetime {
    #[inline]
    fn default() -> Self {
        Self::TransientFrame
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphResourceUsage {
    ColorAttachment,
    DepthAttachment,
    SampledTexture,
    StorageTexture,
    VertexBuffer,
    IndexBuffer,
    UniformBuffer,
    StorageBuffer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphResourceDesc {
    pub id: RenderGraphResourceId,
    pub label: Option<String>,
    pub usage: RenderGraphResourceUsage,
    pub lifetime: RenderGraphResourceLifetime,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphLifetimeStats {
    pub persistent: u32,
    pub transient: u32,
    pub retired: u32,
    pub destroyed: u32,
}
