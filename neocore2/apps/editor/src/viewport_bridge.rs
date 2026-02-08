#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use newengine_ui::draw::UiTexId;

/// Bidirectional UI <-> renderer bridge for the viewport.
///
/// The UI publishes desired pixel extent (physical pixels). The renderer publishes a UI texture id
/// that the UI displays via `egui::TextureId::User(tex_id)`.
///
/// Notes:
/// - The bridge is lock-free; the state fits into atomics.
/// - Width/height are stored as a packed `u64` (`w` in low 32 bits, `h` in high 32 bits).
/// - The UI texture id is stored as `u32` (`newengine_ui::draw::UiTexId`).
#[derive(Debug)]
pub struct ViewportBridge {
    extent_wh: AtomicU64,
    ui_tex: AtomicU32,
}

impl ViewportBridge {
    #[inline]
    pub fn new() -> Self {
        Self {
            extent_wh: AtomicU64::new(0),
            ui_tex: AtomicU32::new(0),
        }
    }

    #[inline]
    fn pack_wh(w: u32, h: u32) -> u64 {
        (w as u64) | ((h as u64) << 32)
    }

    #[inline]
    fn unpack_wh(v: u64) -> (u32, u32) {
        (v as u32, (v >> 32) as u32)
    }

    /// Publish the desired viewport pixel extent from UI.
    #[inline]
    pub fn publish_extent(&self, w: u32, h: u32) {
        self.extent_wh
            .store(Self::pack_wh(w, h), Ordering::Relaxed);
    }

    /// Read the desired viewport pixel extent.
    #[inline]
    pub fn read_extent(&self) -> (u32, u32) {
        Self::unpack_wh(self.extent_wh.load(Ordering::Relaxed))
    }

    /// Publish the UI texture id (renderer -> UI).
    #[inline]
    pub fn publish_ui_tex(&self, tex: Option<UiTexId>) {
        self.ui_tex
            .store(tex.map(|t| t.0).unwrap_or(0), Ordering::Relaxed);
    }

    /// Read the UI texture id (UI -> draw).
    #[inline]
    pub fn read_ui_tex(&self) -> Option<UiTexId> {
        let v = self.ui_tex.load(Ordering::Relaxed);
        if v == 0 {
            None
        } else {
            Some(UiTexId(v))
        }
    }
}
