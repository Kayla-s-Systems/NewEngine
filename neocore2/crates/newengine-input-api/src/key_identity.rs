use super::key_code;

pub const DIGIT1: &str = "keyboard.digit1";
pub const DIGIT2: &str = "keyboard.digit2";
pub const DIGIT3: &str = "keyboard.digit3";
pub const DIGIT4: &str = "keyboard.digit4";
pub const DIGIT5: &str = "keyboard.digit5";
pub const DIGIT6: &str = "keyboard.digit6";
pub const DIGIT7: &str = "keyboard.digit7";
pub const DIGIT8: &str = "keyboard.digit8";

pub const KEY_A: &str = "keyboard.key_a";
pub const KEY_C: &str = "keyboard.key_c";
pub const KEY_D: &str = "keyboard.key_d";
pub const KEY_E: &str = "keyboard.key_e";
pub const KEY_F: &str = "keyboard.key_f";
pub const KEY_I: &str = "keyboard.key_i";
pub const KEY_Q: &str = "keyboard.key_q";
pub const KEY_R: &str = "keyboard.key_r";
pub const KEY_S: &str = "keyboard.key_s";
pub const KEY_W: &str = "keyboard.key_w";
pub const KEY_V: &str = "keyboard.key_v";
pub const KEY_X: &str = "keyboard.key_x";

pub const ENTER: &str = "keyboard.enter";
pub const SPACE: &str = "keyboard.space";
pub const SHIFT_LEFT: &str = "keyboard.shift_left";
pub const SHIFT_RIGHT: &str = "keyboard.shift_right";
pub const CONTROL_LEFT: &str = "keyboard.control_left";
pub const CONTROL_RIGHT: &str = "keyboard.control_right";
pub const TAB: &str = "keyboard.tab";
pub const BACKSPACE: &str = "keyboard.backspace";
pub const DELETE: &str = "keyboard.delete";
pub const HOME: &str = "keyboard.home";
pub const END: &str = "keyboard.end";

pub const ARROW_LEFT: &str = "keyboard.arrow_left";
pub const ARROW_UP: &str = "keyboard.arrow_up";
pub const ARROW_RIGHT: &str = "keyboard.arrow_right";
pub const ARROW_DOWN: &str = "keyboard.arrow_down";

pub const ESCAPE: &str = "keyboard.escape";
pub const F1: &str = "keyboard.f1";
pub const F2: &str = "keyboard.f2";

#[inline]
pub fn key_code_from_id(id: &str) -> Option<u32> {
    match id.trim() {
        DIGIT1 => Some(key_code::DIGIT1),
        DIGIT2 => Some(key_code::DIGIT2),
        DIGIT3 => Some(key_code::DIGIT3),
        DIGIT4 => Some(key_code::DIGIT4),
        DIGIT5 => Some(key_code::DIGIT5),
        DIGIT6 => Some(key_code::DIGIT6),
        DIGIT7 => Some(key_code::DIGIT7),
        DIGIT8 => Some(key_code::DIGIT8),
        KEY_A => Some(key_code::KEY_A),
        KEY_C => Some(key_code::KEY_C),
        KEY_D => Some(key_code::KEY_D),
        KEY_E => Some(key_code::KEY_E),
        KEY_F => Some(key_code::KEY_F),
        KEY_I => Some(key_code::KEY_I),
        KEY_Q => Some(key_code::KEY_Q),
        KEY_R => Some(key_code::KEY_R),
        KEY_S => Some(key_code::KEY_S),
        KEY_W => Some(key_code::KEY_W),
        KEY_V => Some(key_code::KEY_V),
        KEY_X => Some(key_code::KEY_X),
        ENTER => Some(key_code::ENTER),
        SPACE => Some(key_code::SPACE),
        SHIFT_LEFT => Some(key_code::SHIFT_LEFT),
        SHIFT_RIGHT => Some(key_code::SHIFT_RIGHT),
        CONTROL_LEFT => Some(key_code::CONTROL_LEFT),
        CONTROL_RIGHT => Some(key_code::CONTROL_RIGHT),
        TAB => Some(key_code::TAB),
        BACKSPACE => Some(key_code::BACKSPACE),
        DELETE => Some(key_code::DELETE),
        HOME => Some(key_code::HOME),
        END => Some(key_code::END),
        ARROW_LEFT => Some(key_code::ARROW_LEFT),
        ARROW_UP => Some(key_code::ARROW_UP),
        ARROW_RIGHT => Some(key_code::ARROW_RIGHT),
        ARROW_DOWN => Some(key_code::ARROW_DOWN),
        ESCAPE => Some(key_code::ESCAPE),
        F1 => Some(key_code::F1),
        F2 => Some(key_code::F2),
        _ => None,
    }
}

/// Converts native physical key names used by common platform providers into canonical engine ids.
///
/// This function intentionally lives in `newengine-input-api`; platform plugins should not own
/// semantic key ids and must not assign gameplay/editor actions.
#[inline]
pub fn canonical_id_from_native_physical_name(name: &str) -> Option<&'static str> {
    match name.trim() {
        "Digit1" => Some(DIGIT1),
        "Digit2" => Some(DIGIT2),
        "Digit3" => Some(DIGIT3),
        "Digit4" => Some(DIGIT4),
        "Digit5" => Some(DIGIT5),
        "Digit6" => Some(DIGIT6),
        "Digit7" => Some(DIGIT7),
        "Digit8" => Some(DIGIT8),
        "KeyA" => Some(KEY_A),
        "KeyC" => Some(KEY_C),
        "KeyD" => Some(KEY_D),
        "KeyE" => Some(KEY_E),
        "KeyF" => Some(KEY_F),
        "KeyI" => Some(KEY_I),
        "KeyQ" => Some(KEY_Q),
        "KeyR" => Some(KEY_R),
        "KeyS" => Some(KEY_S),
        "KeyW" => Some(KEY_W),
        "KeyV" => Some(KEY_V),
        "KeyX" => Some(KEY_X),
        "Enter" => Some(ENTER),
        "Space" => Some(SPACE),
        "ShiftLeft" => Some(SHIFT_LEFT),
        "ShiftRight" => Some(SHIFT_RIGHT),
        "ControlLeft" => Some(CONTROL_LEFT),
        "ControlRight" => Some(CONTROL_RIGHT),
        "Tab" => Some(TAB),
        "Backspace" => Some(BACKSPACE),
        "Delete" => Some(DELETE),
        "Home" => Some(HOME),
        "End" => Some(END),
        "ArrowLeft" => Some(ARROW_LEFT),
        "ArrowUp" => Some(ARROW_UP),
        "ArrowRight" => Some(ARROW_RIGHT),
        "ArrowDown" => Some(ARROW_DOWN),
        "Escape" => Some(ESCAPE),
        "F1" => Some(F1),
        "F2" => Some(F2),
        _ => None,
    }
}

#[inline]
pub fn key_code_from_native_physical_name(name: &str) -> Option<u32> {
    canonical_id_from_native_physical_name(name).and_then(key_code_from_id)
}
