#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_input_actions_api::{move_mask, CameraViewRequest, InputActionFrame};

/// Mutable carrier used by the render controller so input policy does not own
/// viewport snapshots directly.
pub struct InputActionFrameCarrier<'a> {
    pub dx_px: &'a mut f32,
    pub dy_px: &'a mut f32,
    pub wheel_y: &'a mut f32,
    pub active: &'a mut bool,
    pub look_drag: &'a mut bool,
    pub pan_drag: &'a mut bool,
    pub ui_busy: &'a mut bool,
    pub fly_rmb: &'a mut bool,
    pub move_mask: &'a mut u64,
    pub speed_scalar: &'a mut f32,
    pub camera_view: &'a mut CameraViewRequest,
    pub actions: &'a mut InputActionFrame,
}

impl InputActionFrameCarrier<'_> {
    pub(super) fn suppress_all(&mut self) {
        self.suppress_runtime_controls();
        self.suppress_actions();
    }

    pub(super) fn suppress_actions(&mut self) {
        *self.move_mask = 0;
        *self.camera_view = CameraViewRequest::None;
        *self.actions = InputActionFrame::default();
    }

    pub(super) fn suppress_camera_look(&mut self) {
        *self.dx_px = 0.0;
        *self.dy_px = 0.0;
        self.actions.look_axis = [0.0, 0.0];
    }

    pub(super) fn suppress_gameplay_movement(&mut self) {
        self.actions.move_mask &= !(move_mask::FORWARD
            | move_mask::BACK
            | move_mask::LEFT
            | move_mask::RIGHT
            | move_mask::UP
            | move_mask::DOWN
            | move_mask::SPRINT);
        self.actions.move_axis = [0.0, 0.0, 0.0];
        self.actions.sprint = false;
        *self.move_mask = 0;
        *self.speed_scalar = 1.0;
    }

    pub(super) fn suppress_ui_navigation(&mut self) {
        self.actions.ui_toggle = false;
        self.actions.ui_accept = false;
        self.actions.ui_back = false;
        self.actions.ui_nav = [0, 0];
    }

    pub(super) fn suppress_gamepad_effects(&mut self) {
        self.actions.look_axis = [0.0, 0.0];
        self.suppress_gameplay_movement();
    }

    pub(super) fn suppress_runtime_controls(&mut self) {
        *self.dx_px = 0.0;
        *self.dy_px = 0.0;
        *self.wheel_y = 0.0;
        *self.active = false;
        *self.look_drag = false;
        *self.pan_drag = false;
        *self.ui_busy = true;
        *self.fly_rmb = false;
        *self.move_mask = 0;
        *self.speed_scalar = 1.0;
        *self.camera_view = CameraViewRequest::None;
        self.actions.move_mask = 0;
        self.actions.move_axis = [0.0, 0.0, 0.0];
        self.actions.look_axis = [0.0, 0.0];
        self.actions.sprint = false;
        self.actions.camera_view = CameraViewRequest::None;
    }
}

#[inline]
pub(super) fn action_frame_has_activity(frame: &InputActionFrame) -> bool {
    frame.move_mask != 0
        || frame.move_axis != [0.0, 0.0, 0.0]
        || frame.look_axis != [0.0, 0.0]
        || frame.sprint
        || !matches!(frame.camera_view, CameraViewRequest::None)
        || frame.ui_toggle
        || frame.ui_accept
        || frame.ui_back
        || frame.ui_nav != [0, 0]
        || !frame.actions.is_empty()
}

#[inline]
pub(super) fn movement_has_activity(frame: &InputActionFrame) -> bool {
    frame.move_mask
        & (move_mask::FORWARD
            | move_mask::BACK
            | move_mask::LEFT
            | move_mask::RIGHT
            | move_mask::UP
            | move_mask::DOWN
            | move_mask::SPRINT)
        != 0
        || frame.move_axis != [0.0, 0.0, 0.0]
        || frame.sprint
}
