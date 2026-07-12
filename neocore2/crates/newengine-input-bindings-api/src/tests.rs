use super::*;

struct PressedKey(u32);

impl InputFrameSource for PressedKey {
    fn is_key_down(&self, key: u32) -> bool {
        key == self.0
    }
    fn is_key_pressed(&self, key: u32) -> bool {
        key == self.0
    }
    fn is_key_released(&self, _key: u32) -> bool {
        false
    }
    fn is_mouse_down(&self, _button: u32) -> bool {
        false
    }
    fn is_mouse_pressed(&self, _button: u32) -> bool {
        false
    }
    fn is_mouse_released(&self, _button: u32) -> bool {
        false
    }
}

#[test]
fn profile_canonicalization_merges_defaults_without_duplicate_bindings() {
    let action = InputActionDefinition::new("game.test");
    let mut defaults = InputBindingsProfile::empty("defaults");
    defaults.actions.push(action.clone());
    defaults
        .bindings
        .push(InputBinding::keyboard_pressed("game.test", key_code::KEY_E));
    let mut user = InputBindingsProfile::empty("user");
    user.actions.push(action);
    user.bindings
        .push(InputBinding::keyboard_pressed("game.test", key_code::KEY_E));
    let merged = user.canonicalized_with_defaults(&defaults);
    assert_eq!(merged.bindings.len(), 1);
}

#[test]
fn profile_resolves_registered_keyboard_action() {
    let mut profile = InputBindingsProfile::empty("test");
    profile
        .register_action(InputActionDefinition::new("game.test"))
        .unwrap();
    profile
        .register_binding(InputBindingRegistration::new(
            InputBinding::keyboard_pressed("game.test", key_code::KEY_E),
        ))
        .unwrap();
    let frame = profile.resolve(&PressedKey(key_code::KEY_E));
    assert!(frame.actions.iter().any(|id| id == "game.test"));
}

#[test]
fn display_labels_use_canonical_device_names() {
    assert_eq!(
        binding_display_label(&InputBinding::keyboard_pressed("x", key_code::KEY_E)),
        "E"
    );
    assert_eq!(
        binding_display_label(&InputBinding::gamepad_button_pressed(
            "x",
            gamepad_button::SOUTH
        )),
        "PAD SOUTH"
    );
}
