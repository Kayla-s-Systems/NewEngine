#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Mat4, Vec3};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug)]
pub struct ViewportCameraFrame {
    #[allow(dead_code)]
    pub view: Mat4,
    #[allow(dead_code)]
    pub proj: Mat4,
    pub viewproj: Mat4,
    pub inv_viewproj: Mat4,
    pub vp_w: u32,
    pub vp_h: u32,
}

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

    /// Packed camera input (dx, dy) in *physical pixels* for the last UI frame.
    /// Low 32 bits: f32::to_bits(dx), high 32 bits: f32::to_bits(dy).
    look_delta_xy: AtomicU64,

    /// Mouse wheel delta Y (positive -> zoom in) for the last UI frame.
    /// Stored as f32 bits in low 32 bits.
    wheel_y: AtomicU64,

    /// Orbit interaction flags.
    /// bit0: hovered
    /// bit1: look_drag (camera rotate)
    /// bit2: pan_drag (camera pan)
    /// bit3: ui_busy (UI captured input; e.g. gizmo drag/hover)
    /// bit4: fly_rmb (RMB-held free-fly capture)
    input_flags: AtomicU64,

    /// Packed movement keys for editor-style camera.
    /// bit0: W, bit1: A, bit2: S, bit3: D, bit4: Q, bit5: E, bit6: Shift
    move_keys: AtomicU64,

    /// Selection pick request.
    ///
    /// `pick_seq` is incremented by UI when a new pick is requested.
    /// `pick_xy` stores the cursor position in **physical pixels** relative to the viewport rect.
    pick_seq: AtomicU64,
    pick_xy: AtomicU64,

    /// Explicit camera frame request (UI -> renderer).
    ///
    /// Incremented by UI when the user requests "frame scene" (e.g. hotkey F).
    frame_seq: AtomicU64,

    /// Framing mode for the latest frame request.
    /// 0 = frame selection first, 1 = frame entire scene.
    frame_all: AtomicU64,

    camera_frame: Mutex<Option<ViewportCameraFrame>>,

    /// Latest editor camera state (renderer -> UI).
    ///
    /// Used for deterministic "spawn near camera" placement initiated from UI.
    camera_spawn: Mutex<CameraSpawnState>,
}

#[derive(Clone, Copy, Debug)]
struct CameraSpawnState {
    pos: Vec3,
    forward: Vec3,
}


impl ViewportBridge {
    #[inline]
    pub fn new() -> Self {
        Self {
            extent_wh: AtomicU64::new(0),
            tex_user: AtomicU64::new(0),

            look_delta_xy: AtomicU64::new(0),
            wheel_y: AtomicU64::new(0),
            input_flags: AtomicU64::new(0),
            move_keys: AtomicU64::new(0),

            pick_seq: AtomicU64::new(0),
            pick_xy: AtomicU64::new(0),

            frame_seq: AtomicU64::new(0),
            frame_all: AtomicU64::new(0),

            camera_frame: Mutex::new(None),

            camera_spawn: Mutex::new(CameraSpawnState {
                pos: Vec3::ZERO,
                forward: -Vec3::Z,
            }),
        }
    }

    /// Publish the latest editor camera position and forward direction.
    ///
    /// Called from the render/controller thread once per frame.
    #[inline]
    pub fn publish_camera_spawn(&self, pos: Vec3, forward: Vec3) {
        *self.camera_spawn.lock() = CameraSpawnState {
            pos,
            forward: forward.normalize_or_zero(),
        };
    }

    /// Read the latest camera position and forward direction.
    ///
    /// Called from UI thread when the user requests spawning objects.
    #[inline]
    pub fn read_camera_spawn(&self) -> (Vec3, Vec3) {
        let s = *self.camera_spawn.lock();
        (s.pos, s.forward)
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

    /// Publish camera interaction state from UI.
    ///
    /// - `dx_px`, `dy_px` are cursor deltas in **physical pixels**.
    /// - `wheel_y` is wheel delta Y (positive -> zoom in).
    /// - `hovered` is true when the viewport rect is hovered.
    /// - `look_drag` is true while camera rotation is captured.
    /// - `pan_drag` is true while camera panning is captured.
    #[inline]
    pub fn publish_camera_input(
        &self,
        dx_px: f32,
        dy_px: f32,
        wheel_y: f32,
        hovered: bool,
        look_drag: bool,
        pan_drag: bool,
        ui_busy: bool,
        fly_rmb: bool,
    ) {
        self.look_delta_xy
            .store(Self::pack_f32x2(dx_px, dy_px), Ordering::Relaxed);
        self.wheel_y
            .store(Self::pack_f32(wheel_y), Ordering::Relaxed);
        let mut flags: u64 = 0;
        if hovered {
            flags |= 1;
        }
        if look_drag {
            flags |= 2;
        }
        if pan_drag {
            flags |= 4;
        }
        if ui_busy {
            flags |= 8;
        }
        if fly_rmb {
            flags |= 16;
        }
        self.input_flags.store(flags, Ordering::Relaxed);
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

    /// Read camera input published by UI for this frame.
    #[inline]
    pub fn read_camera_input(&self) -> (f32, f32, f32, bool, bool, bool, bool, bool) {
        let (dx, dy) = Self::unpack_f32x2(self.look_delta_xy.load(Ordering::Relaxed));
        let wheel = Self::unpack_f32(self.wheel_y.load(Ordering::Relaxed));
        let flags = self.input_flags.load(Ordering::Relaxed);
        let hovered = (flags & 1) != 0;
        let look_drag = (flags & 2) != 0;
        let pan_drag = (flags & 4) != 0;
        let ui_busy = (flags & 8) != 0;
        let fly_rmb = (flags & 16) != 0;
        (dx, dy, wheel, hovered, look_drag, pan_drag, ui_busy, fly_rmb)
    }

    /// Publish a pick request from UI.
    ///
    /// `x_px`, `y_px` must be in **physical pixels** relative to the viewport rect.
    #[inline]
    pub fn publish_pick_request(&self, x_px: f32, y_px: f32) {
        self.pick_xy
            .store(Self::pack_f32x2(x_px, y_px), Ordering::Relaxed);
        self.pick_seq.fetch_add(1, Ordering::Relaxed);
    }

    /// Read the latest pick request.
    ///
    /// Returns (seq, x_px, y_px). The caller should track the last processed `seq`.
    #[inline]
    pub fn read_pick_request(&self) -> (u64, f32, f32) {
        let seq = self.pick_seq.load(Ordering::Relaxed);
        let (x, y) = Self::unpack_f32x2(self.pick_xy.load(Ordering::Relaxed));
        (seq, x, y)
    }

    /// Publish an explicit framing request.
    ///
    /// If `all` is true, the renderer will frame the entire scene.
    /// Otherwise it will try to frame selection first.
    #[inline]
    pub fn publish_frame_request(&self, all: bool) {
        self.frame_all.store(all as u64, Ordering::Relaxed);
        self.frame_seq.fetch_add(1, Ordering::Relaxed);
    }

    /// Read the current frame request sequence.
    #[inline]
    pub fn read_frame_request(&self) -> u64 {
        self.frame_seq.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn read_frame_all(&self) -> bool {
        self.frame_all.load(Ordering::Relaxed) != 0
    }

    /// Publish camera matrices and current viewport size (renderer -> UI).
    #[inline]
    pub fn publish_camera_frame(&self, view: Mat4, proj: Mat4, vp_w: u32, vp_h: u32) {
        let viewproj = proj * view;
        let inv_viewproj = viewproj.inverse();
        *self.camera_frame.lock() = Some(ViewportCameraFrame {
            view,
            proj,
            viewproj,
            inv_viewproj,
            vp_w,
            vp_h,
        });
    }

    /// Read last published camera frame (UI).
    #[inline]
    pub fn read_camera_frame(&self) -> Option<ViewportCameraFrame> {
        *self.camera_frame.lock()
    }
}
