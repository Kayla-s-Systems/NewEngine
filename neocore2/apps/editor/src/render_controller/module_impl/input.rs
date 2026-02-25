#![forbid(unsafe_op_in_unsafe_fn)]

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ViewportInputSnap {
    pub dx_px: f32,
    pub dy_px: f32,
    pub wheel_y: f32,

    pub active: bool,

    pub look_drag: bool,
    pub pan_drag: bool,
    pub ui_busy: bool,
    pub fly_rmb: bool,

    pub move_mask: u64,
}

impl ViewportInputSnap {
    #[inline]
    pub(super) fn read(bridge: &crate::viewport_bridge::ViewportBridge) -> Self {
        let (dx_px, dy_px, wheel_y, active, look_drag, pan_drag, ui_busy, fly_rmb, move_mask) =
            bridge.read_camera_input();
        Self {
            dx_px,
            dy_px,
            wheel_y,
            active,
            look_drag,
            pan_drag,
            ui_busy,
            fly_rmb,
            move_mask,
        }
    }

    #[inline]
    pub(super) fn clear_motion(&mut self) {
        self.dx_px = 0.0;
        self.dy_px = 0.0;
        self.wheel_y = 0.0;
    }
}
