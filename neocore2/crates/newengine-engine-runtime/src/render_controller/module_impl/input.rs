#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_input_bindings::{CameraViewRequest, InputBindingsProfile, InputFrameSource};
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
    pub camera_view: CameraViewRequest,
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
            camera_view: CameraViewRequest::None,
        }
    }

    #[inline]
    pub(super) fn read_direct_surface(input: Option<&UiInputFrame>) -> Self {
        let Some(input) = input else {
            return Self::default();
        };
        let actions = InputBindingsProfile::gameplay_default().resolve(&UiInputSource(input));

        Self {
            dx_px: input.mouse_delta.0,
            dy_px: input.mouse_delta.1,
            wheel_y: input.mouse_wheel.1,
            active: true,
            look_drag: true,
            pan_drag: false,
            ui_busy: false,
            fly_rmb: false,
            move_mask: actions.move_mask,
            speed_scalar: 1.0,
            camera_view: actions.camera_view,
        }
    }
}

struct UiInputSource<'a>(&'a UiInputFrame);

impl InputFrameSource for UiInputSource<'_> {
    #[inline]
    fn is_key_down(&self, key: u32) -> bool { self.0.is_key_down(key) }
    #[inline]
    fn is_key_pressed(&self, key: u32) -> bool { self.0.is_key_pressed(key) }
    #[inline]
    fn is_key_released(&self, key: u32) -> bool { self.0.keys_released.contains(&key) }
    #[inline]
    fn is_mouse_down(&self, button: u32) -> bool { self.0.is_mouse_down(button) }
    #[inline]
    fn is_mouse_pressed(&self, button: u32) -> bool { self.0.is_mouse_pressed(button) }
    #[inline]
    fn is_mouse_released(&self, button: u32) -> bool { self.0.mouse_released.contains(&button) }
}
