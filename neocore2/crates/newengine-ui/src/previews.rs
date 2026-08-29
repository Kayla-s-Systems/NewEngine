#![forbid(unsafe_op_in_unsafe_fn)]

use crate::draw::UiTexId;

/// Canonical preview kind.
///
/// The editor may implement custom preview types by mapping them to `Custom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiPreviewKind {
    /// 2D texture preview (may be color-managed / checkerboard alpha).
    Texture2D,
    /// 3D mesh preview (simple lighting, turntable camera).
    Mesh,
    /// Material preview (mesh + material graph).
    Material,
    /// Font glyph atlas preview.
    Font,
    /// Audio waveform / spectrum preview.
    Audio,
    /// User-defined preview type.
    Custom(u32),
}

/// Requested preview descriptor.
///
/// The `key` must be stable across frames (e.g. asset id, path, guid).
/// The preview provider is expected to cache GPU resources internally.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UiPreviewDesc {
    /// Stable key (asset id, guid, path, etc.).
    pub key: Box<str>,
    /// Preview kind.
    pub kind: UiPreviewKind,
    /// Desired output size in pixels.
    pub size_px: [u16; 2],
    /// Optional deterministic seed (for procedural / randomizable previews).
    pub seed: u64,
}

impl UiPreviewDesc {
    #[inline]
    pub fn new(key: impl Into<Box<str>>, kind: UiPreviewKind, size_px: [u16; 2]) -> Self {
        Self {
            key: key.into(),
            kind,
            size_px,
            seed: 0,
        }
    }

    #[inline]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
}

/// A handle to a preview texture.
///
/// Contract:
/// - `tex` is expected to be a **GPU-owned** texture id.
/// - It should remain stable while the preview is alive.
/// - When the preview is no longer used, the provider may recycle its resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiPreviewHandle {
    pub tex: UiTexId,
}

/// Abstract preview provider.
///
/// The host/editor implements this trait and drives the GPU side.
/// UI widgets only request previews and display returned `UiTexId`.
pub trait UiPreviewProvider {
    /// Request (or fetch cached) preview.
    ///
    /// The provider may return a placeholder texture while the preview is being rendered.
    fn request_preview(&mut self, desc: &UiPreviewDesc) -> UiPreviewHandle;

    /// Advance internal preview jobs for the current frame.
    ///
    /// Call once per frame before UI renders. Implementations may kick render graph passes,
    /// resolve transient resources, upload readbacks, etc.
    fn pump_previews(&mut self);
}
