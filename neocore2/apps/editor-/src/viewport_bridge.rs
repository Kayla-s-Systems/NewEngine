#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicU64, Ordering};

/// Bidirectional UI <-> renderer bridge for the viewport.
///
/// The UI publishes desired pixel extent (physical pixels). The renderer publishes an external
/// texture id that the UI displays via `egui::TextureId::User(tex_id)`.
///
/// Notes:
/// - The bridge is lock-free; the state fits into atomics.
/// - Width/height are stored as a packed `u64` (`w` in low 32 bits, `h` in high 32 bits).
/// - `tex_user` is an opaque `u64` passed through unchanged.
#[derive(Debug)]
pub struct ViewportBridge {
    extent_wh: AtomicU64,
    tex_user: AtomicU64,

    /// Packed orbit input (dx, dy) in *physical pixels* for the last UI frame.
    /// Low 32 bits: f32::to_bits(dx), high 32 bits: f32::to_bits(dy).
    orbit_delta_xy: AtomicU64,

    /// Mouse wheel delta Y (positive -> zoom in) for the last UI frame.
    /// Stored as f32 bits in low 32 bits.
    orbit_wheel_y: AtomicU64,

    /// Orbit interaction flags.
    /// bit0: hovered
    /// bit1: lmb_down
    orbit_flags: AtomicU64,

    /// Packed movement keys for editor-style camera.
    /// bit0: W, bit1: A, bit2: S, bit3: D, bit4: Q, bit5: E, bit6: Shift
    move_keys: AtomicU64,
}


impl ViewportBridge {
    #[inline]
    pub fn new() -> Self {
        Self {
            extent_wh: AtomicU64::new(0),
            tex_user: AtomicU64::new(0),

            orbit_delta_xy: AtomicU64::new(0),
            orbit_wheel_y: AtomicU64::new(0),
            orbit_flags: AtomicU64::new(0),
            move_keys: AtomicU64::new(0),
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

    /// Publish the external UI texture id (renderer -> UI).
    #[inline]
    pub fn publish_tex_user(&self, tex_user: u64) {
        self.tex_user.store(tex_user, Ordering::Relaxed);
    }

    /// Read external UI texture id (UI -> draw).
    #[inline]
    pub fn read_tex_user(&self) -> u64 {
        self.tex_user.load(Ordering::Relaxed)
    }

    #[inline]
    fn pack_f32x2(a: f32, b: f32) -> u64 {
        (a.to_bits() as u64) | ((b.to_bits() as u64) << 32)
    }

    #[inline]
    fn unpack_f32x2(v: u64) -> (f32, f32) {
        let a = f32::from_bits(v as u32);
        let b = f32::from_bits((v >> 32) as u32);
        (a, b)
    }

    #[inline]
    fn pack_f32(a: f32) -> u64 {
        a.to_bits() as u64
    }

    #[inline]
    fn unpack_f32(v: u64) -> f32 {
        f32::from_bits(v as u32)
    }

    /// Publish orbit interaction state from UI.
    ///
    /// - `dx_px`, `dy_px` are cursor deltas in **physical pixels**.
    /// - `wheel_y` is wheel delta Y (positive -> zoom in).
    /// - `hovered` is true when the viewport rect is hovered.
    /// - `lmb_down` is true while the primary button is held and the viewport is capturing drag.
    #[inline]
    pub fn publish_orbit_input(
        &self,
        dx_px: f32,
        dy_px: f32,
        wheel_y: f32,
        hovered: bool,
        lmb_down: bool,
    ) {
        self.orbit_delta_xy
            .store(Self::pack_f32x2(dx_px, dy_px), Ordering::Relaxed);
        self.orbit_wheel_y
            .store(Self::pack_f32(wheel_y), Ordering::Relaxed);
        let mut flags: u64 = 0;
        if hovered {
            flags |= 1;
        }
        if lmb_down {
            flags |= 2;
        }
        self.orbit_flags.store(flags, Ordering::Relaxed);
    }


    /// Publish per-frame movement key mask from UI.
    #[inline]
    pub fn publish_move_keys(&self, mask: u64) {
        self.move_keys.store(mask, Ordering::Relaxed);
    }

    /// Read per-frame movement key mask.
    #[inline]
    pub fn read_move_keys(&self) -> u64 {
        self.move_keys.load(Ordering::Relaxed)
    }

    /// Read orbit input published by UI for this frame.
    #[inline]
    pub fn read_orbit_input(&self) -> (f32, f32, f32, bool, bool) {
        let (dx, dy) = Self::unpack_f32x2(self.orbit_delta_xy.load(Ordering::Relaxed));
        let wheel = Self::unpack_f32(self.orbit_wheel_y.load(Ordering::Relaxed));
        let flags = self.orbit_flags.load(Ordering::Relaxed);
        let hovered = (flags & 1) != 0;
        let lmb_down = (flags & 2) != 0;
        (dx, dy, wheel, hovered, lmb_down)
    }
}
