#![forbid(unsafe_op_in_unsafe_fn)]

use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use newengine_math::collections::FxHashMap;

mod binary;
mod paint;
pub use binary::{decode_ui_draw_list_bin, encode_ui_draw_list_bin, encode_ui_draw_list_bin_into};
pub use paint::{
    TextureRef, UiBorderPaintCommand, UiClipPaintCommand, UiIconPaintCommand, UiImagePaintCommand,
    UiImageRef, UiLayerPaintCommand, UiPaintCommand, UiPaintList, UiPaintNodeRef,
    UiRectPaintCommand, UiRoundedRectPaintCommand, UiScopePaintCommand, UiTextPaintCommand,
    UiVectorPaintCommand, VectorRef,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct UiTexId(pub u32);

pub mod reserved {
    use super::UiTexId;

    pub const FONT_ATLAS: UiTexId = UiTexId(1);
    /// Engine-owned 1x1 white texture used by renderer-side paint command fallback
    /// tessellation for rects, borders and temporary vector stubs.
    pub const SOLID_WHITE: UiTexId = UiTexId(15);
    pub const USER_BEGIN: u32 = 16;

    /// Reserved range for external GPU-owned textures.
    ///
    /// Contract:
    /// - user-space ids start at `USER_BEGIN`
    /// - external textures occupy the high-bit namespace
    /// - engine-managed ids must never collide with external ids
    pub const EXTERNAL_BEGIN: u32 = 0x8000_0000;

    #[inline]
    pub const fn external_from_u32(local: u32) -> UiTexId {
        UiTexId(EXTERNAL_BEGIN | (local & 0x7FFF_FFFF))
    }

    #[inline]
    pub const fn is_external(id: UiTexId) -> bool {
        (id.0 & EXTERNAL_BEGIN) != 0
    }
}
impl UiTexId {
    #[inline]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(C)]
pub struct UiRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl UiRect {
    #[inline]
    pub fn empty() -> Self {
        Self {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.max_x <= self.min_x || self.max_y <= self.min_y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
#[repr(C)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiDrawCmd {
    pub texture: UiTexId,
    pub clip_rect: UiRect,
    pub index_range: core::ops::Range<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiMesh {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
    pub cmds: SmallVec<[UiDrawCmd; 8]>,
}

impl UiMesh {
    #[inline]
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            cmds: SmallVec::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.cmds.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiDrawList {
    pub screen_size_px: [u32; 2],
    pub pixels_per_point: f32,
    /// Legacy/backend-ready triangle mesh stream. Existing render backends may
    /// keep consuming this while the Vulkan UI renderer migrates to paint commands.
    pub mesh: UiMesh,
    /// Renderer-neutral command stream. Aurelia owns layout/input/state and emits
    /// generic primitives here; GPU backends turn this into batches.
    pub paint: UiPaintList,
    pub texture_delta: UiTextureDelta,
}

impl UiDrawList {
    #[inline]
    pub fn new() -> Self {
        Self {
            screen_size_px: [0, 0],
            pixels_per_point: 1.0,
            mesh: UiMesh::new(),
            paint: UiPaintList::new(),
            texture_delta: UiTextureDelta::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.mesh.clear();
        self.paint.clear();
        self.texture_delta.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTexture {
    pub size: [u32; 2],
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTextureDelta {
    pub set: FxHashMap<UiTexId, UiTexture>,
    pub patches: Vec<UiTexturePatch>,
    pub free: Vec<UiTexId>,
}

impl UiTextureDelta {
    #[inline]
    pub fn new() -> Self {
        Self {
            set: FxHashMap::default(),
            patches: Vec::new(),
            free: Vec::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.set.clear();
        self.patches.clear();
        self.free.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTexturePatch {
    pub id: UiTexId,
    pub origin: [u32; 2],
    pub size: [u32; 2],
    pub rgba8: Vec<u8>,
}
