#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_input_actions_api::{CameraViewRequest, InputActionFrame, InputFrameSource};
use newengine_ui::UiInputFrame;
use crate::input_systems::InputActionFrameCarrier;

#[derive(Clone, Debug, Default)]
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
    pub actions: InputActionFrame,
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
            actions: InputActionFrame::default(),
        }
    }

    #[inline]
    pub(super) fn read_direct_surface(input: Option<&UiInputFrame>) -> Self {
        let Some(input) = input else {
            return Self::default();
        };
        let actions = newengine_input_bindings_runtime::resolve_input_actions(&UiInputSource(input));

        Self {
            dx_px: input.mouse_delta.0 + actions.look_axis[0] * 18.0,
            dy_px: input.mouse_delta.1 + actions.look_axis[1] * 18.0,
            wheel_y: input.mouse_wheel.1,
            active: true,
            look_drag: true,
            pan_drag: false,
            ui_busy: false,
            fly_rmb: false,
            move_mask: actions.move_mask,
            speed_scalar: 1.0,
            camera_view: actions.camera_view,
            actions,
        }
    }


    #[inline]
    pub(super) fn action_carrier(&mut self) -> InputActionFrameCarrier<'_> {
        InputActionFrameCarrier {
            dx_px: &mut self.dx_px,
            dy_px: &mut self.dy_px,
            wheel_y: &mut self.wheel_y,
            active: &mut self.active,
            look_drag: &mut self.look_drag,
            pan_drag: &mut self.pan_drag,
            ui_busy: &mut self.ui_busy,
            fly_rmb: &mut self.fly_rmb,
            move_mask: &mut self.move_mask,
            speed_scalar: &mut self.speed_scalar,
            camera_view: &mut self.camera_view,
            actions: &mut self.actions,
        }
    }

    #[inline]
    pub(super) fn suppress_runtime_controls(&mut self) {
        self.dx_px = 0.0;
        self.dy_px = 0.0;
        self.wheel_y = 0.0;
        self.active = false;
        self.look_drag = false;
        self.pan_drag = false;
        self.ui_busy = true;
        self.fly_rmb = false;
        self.move_mask = 0;
        self.speed_scalar = 1.0;
        self.camera_view = CameraViewRequest::None;
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
    #[inline]
    fn has_gamepad_connected(&self) -> bool { self.0.has_gamepad_connected() }

    #[inline]
    fn is_gamepad_button_down(&self, button: &str) -> bool { self.0.is_gamepad_button_down(button) }
    #[inline]
    fn is_gamepad_button_pressed(&self, button: &str) -> bool { self.0.is_gamepad_button_pressed(button) }
    #[inline]
    fn is_gamepad_button_released(&self, button: &str) -> bool { self.0.is_gamepad_button_released(button) }
    #[inline]
    fn gamepad_axis(&self, axis: &str) -> f32 { self.0.gamepad_axis(axis) }
}
