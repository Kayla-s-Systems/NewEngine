use crate::types::Asset;

/// CPU-side texture payload.
///
/// Designed to be uploaded to GPU without additional processing.
/// Supports uncompressed RGBA8 and common BCn block-compressed formats.
/// For DDS cubemaps/arrays you get `layers > 1`.
#[derive(Debug, Clone)]
pub struct TextureAsset {
    pub desc: TextureDesc,
    pub mips: Vec<TextureMip>,
}

/// Texture description (independent of any graphics backend).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub layers: u32,
    pub mip_count: u32,
    pub format: TextureFormat,
    pub kind: TextureKind,
}

/// Texture kind (2D/3D/Cube).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureKind {
    Tex2D,
    Tex3D,
    Cube,
}

/// Texture pixel/block format.
///
/// This is intentionally small; extend as needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8Unorm,
    Bc1RgbUnorm,
    Bc1RgbaUnorm,
    Bc2Unorm,
    Bc3Unorm,
    Bc4Unorm,
    Bc5Unorm,
    Bc7Unorm,
}

/// One mip level for a single layer.
/// The store is "layer-major": all mip0 layers, then mip1 layers, etc (or vice versa).
/// We choose: mips are stored as `mip_index` entries, each containing `layers` slices in `subresources`.
#[derive(Debug, Clone)]
pub struct TextureMip {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub subresources: Vec<TextureSubresource>,
}

/// One subresource: (layer, possibly depth slice packed) payload.
#[derive(Debug, Clone)]
pub struct TextureSubresource {
    pub layer: u32,
    pub data: Vec<u8>,
}

impl Asset for TextureAsset {
    #[inline]
    fn type_name() -> &'static str {
        "TextureAsset"
    }
}