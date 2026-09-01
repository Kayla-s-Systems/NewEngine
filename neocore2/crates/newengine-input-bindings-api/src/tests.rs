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

struct PressedMouse(u32);

impl InputFrameSource for PressedMouse {
    fn is_key_down(&self, _key: u32) -> bool {
        false
    }
    fn is_key_pressed(&self, _key: u32) -> bool {
        false
    }
    fn is_key_released(&self, _key: u32) -> bool {
        false
    }
    fn is_mouse_down(&self, button: u32) -> bool {
        button == self.0
    }
    fn is_mouse_pressed(&self, button: u32) -> bool {
        button == self.0
    }
    fn is_mouse_released(&self, _button: u32) -> bool {
        false
    }
}

#[test]
fn profile_preserves_down_and_pressed_phases_for_same_action() {
    let mut profile = InputBindingsProfile::empty("phase-regression");
    profile
        .register_action(InputActionDefinition::new("player.fire.primary"))
        .unwrap();
    profile
        .register_binding(InputBindingRegistration::new(
            InputBinding::mouse_button_down(
                "player.fire.primary",
                newengine_input_api::mouse_button::LEFT,
            ),
        ))
        .unwrap();
    profile
        .register_binding(InputBindingRegistration::new(
            InputBinding::mouse_button_pressed(
                "player.fire.primary",
                newengine_input_api::mouse_button::LEFT,
            ),
        ))
        .unwrap();

    let frame = profile.resolve(&PressedMouse(newengine_input_api::mouse_button::LEFT));
    let commands = frame.command_actions();
    assert!(commands.is_held("player.fire.primary"));
    assert!(commands.is_pressed("player.fire.primary"));
    assert_eq!(
        frame
            .actions
            .iter()
            .filter(|id| id.as_str() == "player.fire.primary")
            .count(),
        1
    );
}

#[derive(Default)]
struct GamepadFrame {
    connected: bool,
    down: std::collections::BTreeSet<String>,
    pressed: std::collections::BTreeSet<String>,
    released: std::collections::BTreeSet<String>,
    axes: std::collections::BTreeMap<String, f32>,
}

impl InputFrameSource for GamepadFrame {
    fn is_key_down(&self, _key: u32) -> bool {
        false
    }
    fn is_key_pressed(&self, _key: u32) -> bool {
        false
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
    fn has_gamepad_connected(&self) -> bool {
        self.connected
    }
    fn is_gamepad_button_down(&self, button: &str) -> bool {
        self.down.contains(button)
    }
    fn is_gamepad_button_pressed(&self, button: &str) -> bool {
        self.pressed.contains(button)
    }
    fn is_gamepad_button_released(&self, button: &str) -> bool {
        self.released.contains(button)
    }
    fn gamepad_axis(&self, axis: &str) -> f32 {
        self.axes.get(axis).copied().unwrap_or(0.0)
    }
}

#[test]
fn gamepad_buttons_and_axes_resolve_into_semantic_action_frame() {
    let mut profile = InputBindingsProfile::empty("gamepad-regression");
    profile
        .register_action(InputActionDefinition::new("player.jump"))
        .unwrap();
    profile
        .register_binding(InputBindingRegistration::new(
            InputBinding::gamepad_button_pressed("player.jump", gamepad_button::SOUTH),
        ))
        .unwrap();
    profile
        .register_gamepad_axis(GamepadAxisBinding::new(
            gamepad_axis::LEFT_STICK_X,
            GamepadAxisTarget::MoveX,
            1.0,
        ))
        .unwrap();
    profile
        .register_gamepad_axis(GamepadAxisBinding::new(
            gamepad_axis::LEFT_STICK_Y,
            GamepadAxisTarget::MoveZ,
            -1.0,
        ))
        .unwrap();
    profile
        .register_gamepad_axis(GamepadAxisBinding::new(
            gamepad_axis::RIGHT_STICK_X,
            GamepadAxisTarget::LookX,
            1.0,
        ))
        .unwrap();

    let mut input = GamepadFrame {
        connected: true,
        ..GamepadFrame::default()
    };
    input.pressed.insert(gamepad_button::SOUTH.to_owned());
    input
        .axes
        .insert(gamepad_axis::LEFT_STICK_X.to_owned(), 0.75);
    input
        .axes
        .insert(gamepad_axis::LEFT_STICK_Y.to_owned(), -0.80);
    input
        .axes
        .insert(gamepad_axis::RIGHT_STICK_X.to_owned(), 0.50);

    let frame = profile.resolve(&input);
    assert!(frame.actions.iter().any(|action| action == "player.jump"));
    assert!(frame.command_actions().is_pressed("player.jump"));
    assert!((frame.move_axis[0] - 0.75).abs() < 0.0001);
    assert!((frame.move_axis[2] - 0.80).abs() < 0.0001);
    assert!((frame.look_axis[0] - 0.50).abs() < 0.0001);
    assert_ne!(
        frame.move_mask & newengine_input_actions_api::move_mask::RIGHT,
        0
    );
    assert_ne!(
        frame.move_mask & newengine_input_actions_api::move_mask::FORWARD,
        0
    );
}
