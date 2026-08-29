use super::*;

#[inline]
pub fn gameplay_default_gamepad_axes() -> Vec<GamepadAxisBinding> {
    vec![
        GamepadAxisBinding::new(gamepad_axis::LEFT_STICK_X, GamepadAxisTarget::MoveX, 1.0),
        GamepadAxisBinding::new(gamepad_axis::LEFT_STICK_Y, GamepadAxisTarget::MoveZ, -1.0),
        GamepadAxisBinding::new(gamepad_axis::RIGHT_STICK_X, GamepadAxisTarget::LookX, 1.0),
        GamepadAxisBinding::new(gamepad_axis::RIGHT_STICK_Y, GamepadAxisTarget::LookY, -1.0),
    ]
}
