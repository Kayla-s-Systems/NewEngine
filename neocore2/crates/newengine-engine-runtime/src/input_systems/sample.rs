#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ui_api::UiInputFrame;

use super::{input_has_pause_menu_action, InputActionFrameCarrier};

#[derive(Clone, Copy, Debug)]
pub(super) struct RawInputSample {
    pub(super) present: bool,
    pub(super) keys_down: usize,
    pub(super) keys_pressed: usize,
    pub(super) mouse_motion: bool,
    pub(super) gamepad_connected: usize,
    pub(super) gamepad_activity: bool,
}

impl RawInputSample {
    pub(super) fn from_surface(surface_input: Option<&UiInputFrame>) -> Self {
        surface_input
            .map(|frame| Self {
                present: true,
                keys_down: frame.keys_down.len(),
                keys_pressed: frame.keys_pressed.len(),
                mouse_motion: frame.mouse_delta.0.abs() > f32::EPSILON
                    || frame.mouse_delta.1.abs() > f32::EPSILON,
                gamepad_connected: frame.gamepad_connected,
                gamepad_activity: frame.has_gamepad_activity(),
            })
            .unwrap_or(Self {
                present: false,
                keys_down: 0,
                keys_pressed: 0,
                mouse_motion: false,
                gamepad_connected: 0,
                gamepad_activity: false,
            })
    }

    pub(super) fn summary(self, input: &InputActionFrameCarrier<'_>) -> String {
        format!(
            "raw={} keys_down={} keys_pressed={} mouse_motion={} gamepads={} gamepad_activity={} actions={} move=0x{:X} look=({:.2},{:.2}) menu={}",
            self.present,
            self.keys_down,
            self.keys_pressed,
            self.mouse_motion,
            self.gamepad_connected,
            self.gamepad_activity,
            input.actions.actions.len(),
            input.actions.move_mask,
            input.actions.look_axis[0],
            input.actions.look_axis[1],
            input_has_pause_menu_action(input),
        )
    }
}
