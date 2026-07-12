use super::*;

#[inline]
pub fn binding_display_label(binding: &InputBinding) -> String {
    match binding.device {
        InputBindingDevice::Keyboard => key_code_label(binding.code),
        InputBindingDevice::MouseButton => mouse_button_label(binding.code),
        InputBindingDevice::GamepadButton => binding
            .name
            .as_deref()
            .map(gamepad_button_label)
            .unwrap_or("GAMEPAD"),
    }
    .to_owned()
}

#[inline]
pub fn key_code_label(code: u32) -> &'static str {
    match code {
        key_code::DIGIT1 => "1",
        key_code::DIGIT2 => "2",
        key_code::DIGIT3 => "3",
        key_code::KEY_A => "A",
        key_code::KEY_D => "D",
        key_code::KEY_E => "E",
        key_code::KEY_F => "F",
        key_code::KEY_Q => "Q",
        key_code::KEY_S => "S",
        key_code::KEY_W => "W",
        key_code::ENTER => "ENTER",
        key_code::SPACE => "SPACE",
        key_code::SHIFT_LEFT => "LEFT SHIFT",
        key_code::SHIFT_RIGHT => "RIGHT SHIFT",
        key_code::TAB => "TAB",
        key_code::BACKSPACE => "BACKSPACE",
        key_code::ARROW_LEFT => "LEFT",
        key_code::ARROW_UP => "UP",
        key_code::ARROW_RIGHT => "RIGHT",
        key_code::ARROW_DOWN => "DOWN",
        key_code::ESCAPE => "ESC",
        key_code::F1 => "F1",
        key_code::F2 => "F2",
        _ => "KEY",
    }
}

#[inline]
pub fn mouse_button_label(code: u32) -> &'static str {
    match code {
        1 => "MOUSE LEFT",
        2 => "MOUSE RIGHT",
        3 => "MOUSE MIDDLE",
        4 => "MOUSE BACK",
        5 => "MOUSE FORWARD",
        _ => "MOUSE",
    }
}

#[inline]
pub fn gamepad_button_label(name: &str) -> &'static str {
    match name {
        gamepad_button::SOUTH => "PAD SOUTH",
        gamepad_button::EAST => "PAD EAST",
        gamepad_button::WEST => "PAD WEST",
        gamepad_button::NORTH => "PAD NORTH",
        gamepad_button::LEFT_THUMB => "PAD L3",
        gamepad_button::RIGHT_THUMB => "PAD R3",
        gamepad_button::START => "PAD START",
        gamepad_button::SELECT => "PAD SELECT",
        gamepad_button::MODE => "PAD MODE",
        gamepad_button::DPAD_UP => "DPAD UP",
        gamepad_button::DPAD_DOWN => "DPAD DOWN",
        gamepad_button::DPAD_LEFT => "DPAD LEFT",
        gamepad_button::DPAD_RIGHT => "DPAD RIGHT",
        _ => "GAMEPAD",
    }
}
