#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ui::input::keys as ui_keys;
use newengine_ui::UiInputFrame;

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
    pub speed_scalar: f32,
}

impl ViewportInputSnap {
    #[inline]
    pub(super) fn read(bridge: &crate::viewport_bridge::ViewportBridge) -> Self {
        let (dx_px, dy_px, wheel_y, active, look_drag, pan_drag, ui_busy, fly_rmb, move_mask, speed_scalar) =
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
            speed_scalar,
        }
    }

    #[inline]
    pub(super) fn read_direct_surface(input: Option<&UiInputFrame>) -> Self {
        let Some(input) = input else {
            return Self::default();
        };

        let mut move_mask: u64 = 0;
        if input.is_key_down(ui_keys::KEY_W) {
            move_mask |= newengine_viewport::input::MOVE_W;
        }
        if input.is_key_down(ui_keys::KEY_A) {
            move_mask |= newengine_viewport::input::MOVE_A;
        }
        if input.is_key_down(ui_keys::KEY_S) {
            move_mask |= newengine_viewport::input::MOVE_S;
        }
        if input.is_key_down(ui_keys::KEY_D) {
            move_mask |= newengine_viewport::input::MOVE_D;
        }
        if input.is_key_down(ui_keys::KEY_Q) {
            move_mask |= newengine_viewport::input::MOVE_UP;
        }
        if input.is_key_down(ui_keys::KEY_E) {
            move_mask |= newengine_viewport::input::MOVE_DOWN;
        }
        if input.is_key_down(ui_keys::SHIFT_LEFT) || input.is_key_down(ui_keys::SHIFT_RIGHT) {
            move_mask |= newengine_viewport::input::MOVE_SHIFT;
        }

        Self {
            dx_px: input.mouse_delta.0,
            dy_px: input.mouse_delta.1,
            wheel_y: input.mouse_wheel.1,
            active: true,
            look_drag: true,
            pan_drag: false,
            ui_busy: false,
            fly_rmb: false,
            move_mask,
            speed_scalar: 1.0,
        }
    }
}
