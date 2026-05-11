use crate::{BindGroupLayoutId, BufferId, SamplerId, TextureId};
use serde::{Deserialize, Serialize};

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
    pub texture1: Option<TextureId>,
    pub texture2: Option<TextureId>,
    pub texture3: Option<TextureId>,
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
            texture1: None,
            texture2: None,
            texture3: None,
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
    pub fn with_texture1(mut self, tex: TextureId) -> Self {
        self.texture1 = Some(tex);
        self
    }

    #[inline]
    pub fn with_texture2(mut self, tex: TextureId) -> Self {
        self.texture2 = Some(tex);
        self
    }

    #[inline]
    pub fn with_texture3(mut self, tex: TextureId) -> Self {
        self.texture3 = Some(tex);
        self
    }

    #[inline]
    pub fn texture_at(&self, index: usize) -> Option<TextureId> {
        match index {
            0 => self.texture0,
            1 => self.texture1,
            2 => self.texture2,
            3 => self.texture3,
            _ => None,
        }
    }

    #[inline]
    pub fn with_sampler0(mut self, sampler: SamplerId) -> Self {
        self.sampler0 = Some(sampler);
        self
    }

    #[inline]
    pub fn with_uniform0(mut self, binding: BufferBinding) -> Self {
        self.uniform0 = Some(binding);
        self
    }

    #[inline]
    pub fn with_storage0(mut self, binding: BufferBinding) -> Self {
        self.storage0 = Some(binding);
        self
    }
}
