use super::*;

#[inline]
pub fn gameplay_default_key_registry() -> Vec<InputKeyRegistration> {
    use newengine_input_api::key_code as keys;
    vec![
        InputKeyRegistration::new(keys::DIGIT1, key_identity::DIGIT1, "1"),
        InputKeyRegistration::new(keys::DIGIT2, key_identity::DIGIT2, "2"),
        InputKeyRegistration::new(keys::DIGIT3, key_identity::DIGIT3, "3"),
        InputKeyRegistration::new(keys::DIGIT4, key_identity::DIGIT4, "4"),
        InputKeyRegistration::new(keys::DIGIT5, key_identity::DIGIT5, "5"),
        InputKeyRegistration::new(keys::DIGIT6, key_identity::DIGIT6, "6"),
        InputKeyRegistration::new(keys::DIGIT7, key_identity::DIGIT7, "7"),
        InputKeyRegistration::new(keys::DIGIT8, key_identity::DIGIT8, "8"),
        InputKeyRegistration::new(keys::KEY_A, key_identity::KEY_A, "A"),
        InputKeyRegistration::new(keys::KEY_C, key_identity::KEY_C, "C"),
        InputKeyRegistration::new(keys::KEY_D, key_identity::KEY_D, "D"),
        InputKeyRegistration::new(keys::KEY_E, key_identity::KEY_E, "E"),
        InputKeyRegistration::new(keys::KEY_F, key_identity::KEY_F, "F"),
        InputKeyRegistration::new(keys::KEY_I, key_identity::KEY_I, "I"),
        InputKeyRegistration::new(keys::KEY_M, key_identity::KEY_M, "M"),
        InputKeyRegistration::new(keys::KEY_V, key_identity::KEY_V, "V"),
        InputKeyRegistration::new(keys::KEY_Q, key_identity::KEY_Q, "Q"),
        InputKeyRegistration::new(keys::KEY_R, key_identity::KEY_R, "R"),
        InputKeyRegistration::new(keys::KEY_S, key_identity::KEY_S, "S"),
        InputKeyRegistration::new(keys::KEY_W, key_identity::KEY_W, "W"),
        InputKeyRegistration::new(keys::ENTER, key_identity::ENTER, "ENTER"),
        InputKeyRegistration::new(keys::SPACE, key_identity::SPACE, "SPACE"),
        InputKeyRegistration::new(keys::SHIFT_LEFT, key_identity::SHIFT_LEFT, "LEFT SHIFT"),
        InputKeyRegistration::new(keys::SHIFT_RIGHT, key_identity::SHIFT_RIGHT, "RIGHT SHIFT"),
        InputKeyRegistration::new(keys::CONTROL_LEFT, key_identity::CONTROL_LEFT, "LEFT CTRL"),
        InputKeyRegistration::new(
            keys::CONTROL_RIGHT,
            key_identity::CONTROL_RIGHT,
            "RIGHT CTRL",
        ),
        InputKeyRegistration::new(keys::TAB, key_identity::TAB, "TAB"),
        InputKeyRegistration::new(keys::BACKSPACE, key_identity::BACKSPACE, "BACKSPACE"),
        InputKeyRegistration::new(keys::ARROW_LEFT, key_identity::ARROW_LEFT, "LEFT"),
        InputKeyRegistration::new(keys::ARROW_UP, key_identity::ARROW_UP, "UP"),
        InputKeyRegistration::new(keys::ARROW_RIGHT, key_identity::ARROW_RIGHT, "RIGHT"),
        InputKeyRegistration::new(keys::ARROW_DOWN, key_identity::ARROW_DOWN, "DOWN"),
        InputKeyRegistration::new(keys::ESCAPE, key_identity::ESCAPE, "ESC"),
        InputKeyRegistration::new(keys::F1, key_identity::F1, "F1"),
        InputKeyRegistration::new(keys::F2, key_identity::F2, "F2"),
    ]
}
