#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_input_actions_api::{move_mask, CameraViewRequest, InputActionDefinition, InputActionDispatchMode, InputActionEffect, InputActionListenerRegistration};
use newengine_input_api::{gamepad_axis, gamepad_button, key_code};
use newengine_input_bindings_api::{GamepadAxisBinding, GamepadAxisTarget, InputBinding, InputBindingDevice, InputBindingPhase, InputBindingsProfile, InputDevicePreference, InputKeyRegistration};

pub mod action {
    pub const PLAYER_MOVE_FORWARD: &str = "player.move.forward";
    pub const PLAYER_MOVE_BACK: &str = "player.move.back";
    pub const PLAYER_MOVE_LEFT: &str = "player.move.left";
    pub const PLAYER_MOVE_RIGHT: &str = "player.move.right";
    pub const PLAYER_MOVE_UP: &str = "player.move.up";
    pub const PLAYER_MOVE_DOWN: &str = "player.move.down";
    pub const PLAYER_SPRINT: &str = "player.sprint";

    pub const CAMERA_VIEW_NEXT: &str = "camera.view.next";
    pub const CAMERA_VIEW_PREVIOUS: &str = "camera.view.previous";
    pub const CAMERA_VIEW_FIRST_PERSON: &str = "camera.view.first_person";
    pub const CAMERA_VIEW_THIRD_PERSON_FOLLOW: &str = "camera.view.third_person.follow";
    pub const CAMERA_VIEW_THIRD_PERSON_AIM: &str = "camera.view.third_person.aim";

    pub const UI_MENU_TOGGLE: &str = "engine.ui.menu.toggle_pause";
    pub const UI_MENU_ACCEPT: &str = "ui.menu.accept";
    pub const UI_MENU_BACK: &str = "ui.menu.back";
    pub const UI_MENU_UP: &str = "ui.menu.up";
    pub const UI_MENU_DOWN: &str = "ui.menu.down";
    pub const UI_MENU_LEFT: &str = "ui.menu.left";
    pub const UI_MENU_RIGHT: &str = "ui.menu.right";
}

#[inline]
pub fn game_ready_input_profile() -> InputBindingsProfile {
    let mut bindings = gameplay_default_bindings();
    ensure_required_system_bindings(&mut bindings);
    InputBindingsProfile {
        id: "newengine.gameready.input.profile".to_owned(),
        version: 4,
        device_preference: InputDevicePreference::Hybrid,
        keys: gameplay_default_key_registry(),
        actions: gameplay_default_actions(),
        listeners: gameplay_default_listeners(),
        bindings,
        gamepad_axes: gameplay_default_gamepad_axes(),
    }
    .canonicalized()
}

#[inline]
pub fn gameplay_default_key_registry() -> Vec<InputKeyRegistration> {
    use newengine_input_api::key_code as keys;
    vec![
        InputKeyRegistration::new(keys::DIGIT1, "keyboard.digit1", "1"),
        InputKeyRegistration::new(keys::DIGIT2, "keyboard.digit2", "2"),
        InputKeyRegistration::new(keys::DIGIT3, "keyboard.digit3", "3"),
        InputKeyRegistration::new(keys::KEY_A, "keyboard.key_a", "A"),
        InputKeyRegistration::new(keys::KEY_D, "keyboard.key_d", "D"),
        InputKeyRegistration::new(keys::KEY_E, "keyboard.key_e", "E"),
        InputKeyRegistration::new(keys::KEY_F, "keyboard.key_f", "F"),
        InputKeyRegistration::new(keys::KEY_Q, "keyboard.key_q", "Q"),
        InputKeyRegistration::new(keys::KEY_S, "keyboard.key_s", "S"),
        InputKeyRegistration::new(keys::KEY_W, "keyboard.key_w", "W"),
        InputKeyRegistration::new(keys::ENTER, "keyboard.enter", "ENTER"),
        InputKeyRegistration::new(keys::SPACE, "keyboard.space", "SPACE"),
        InputKeyRegistration::new(keys::SHIFT_LEFT, "keyboard.shift_left", "LEFT SHIFT"),
        InputKeyRegistration::new(keys::SHIFT_RIGHT, "keyboard.shift_right", "RIGHT SHIFT"),
        InputKeyRegistration::new(keys::TAB, "keyboard.tab", "TAB"),
        InputKeyRegistration::new(keys::BACKSPACE, "keyboard.backspace", "BACKSPACE"),
        InputKeyRegistration::new(keys::ARROW_LEFT, "keyboard.arrow_left", "LEFT"),
        InputKeyRegistration::new(keys::ARROW_UP, "keyboard.arrow_up", "UP"),
        InputKeyRegistration::new(keys::ARROW_RIGHT, "keyboard.arrow_right", "RIGHT"),
        InputKeyRegistration::new(keys::ARROW_DOWN, "keyboard.arrow_down", "DOWN"),
        InputKeyRegistration::new(keys::ESCAPE, "keyboard.escape", "ESC"),
    ]
}

#[inline]
pub fn gameplay_default_actions() -> Vec<InputActionDefinition> {
    vec![
        InputActionDefinition::new(action::PLAYER_MOVE_FORWARD).with_label("Move forward").with_effect(InputActionEffect::MoveMask { mask: move_mask::FORWARD }),
        InputActionDefinition::new(action::PLAYER_MOVE_BACK).with_label("Move back").with_effect(InputActionEffect::MoveMask { mask: move_mask::BACK }),
        InputActionDefinition::new(action::PLAYER_MOVE_LEFT).with_label("Move left").with_effect(InputActionEffect::MoveMask { mask: move_mask::LEFT }),
        InputActionDefinition::new(action::PLAYER_MOVE_RIGHT).with_label("Move right").with_effect(InputActionEffect::MoveMask { mask: move_mask::RIGHT }),
        InputActionDefinition::new(action::PLAYER_MOVE_UP).with_label("Move up").with_effect(InputActionEffect::MoveMask { mask: move_mask::UP }),
        InputActionDefinition::new(action::PLAYER_MOVE_DOWN).with_label("Move down").with_effect(InputActionEffect::MoveMask { mask: move_mask::DOWN }),
        InputActionDefinition::new(action::PLAYER_SPRINT).with_label("Sprint").with_effect(InputActionEffect::MoveMask { mask: move_mask::SPRINT }).with_effect(InputActionEffect::Sprint { enabled: true }),
        InputActionDefinition::new(action::CAMERA_VIEW_NEXT).with_label("Next camera view").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Next }),
        InputActionDefinition::new(action::CAMERA_VIEW_PREVIOUS).with_label("Previous camera view").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Previous }),
        InputActionDefinition::new(action::CAMERA_VIEW_FIRST_PERSON).with_label("First-person camera").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Set(newengine_camera_api::CameraViewMode::FirstPerson) }),
        InputActionDefinition::new(action::CAMERA_VIEW_THIRD_PERSON_FOLLOW).with_label("Third-person follow camera").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Set(newengine_camera_api::CameraViewMode::ThirdPersonFollow) }),
        InputActionDefinition::new(action::CAMERA_VIEW_THIRD_PERSON_AIM).with_label("Third-person aim camera").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Set(newengine_camera_api::CameraViewMode::ThirdPersonAim) }),
        InputActionDefinition::new(action::UI_MENU_TOGGLE).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Toggle menu").with_effect(InputActionEffect::MenuToggle),
        InputActionDefinition::new(action::UI_MENU_ACCEPT).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Accept").with_effect(InputActionEffect::MenuAccept),
        InputActionDefinition::new(action::UI_MENU_BACK).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Back").with_effect(InputActionEffect::MenuBack),
        InputActionDefinition::new(action::UI_MENU_UP).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Menu up").with_effect(InputActionEffect::MenuNav { x: 0, y: -1 }),
        InputActionDefinition::new(action::UI_MENU_DOWN).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Menu down").with_effect(InputActionEffect::MenuNav { x: 0, y: 1 }),
        InputActionDefinition::new(action::UI_MENU_LEFT).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Menu left").with_effect(InputActionEffect::MenuNav { x: -1, y: 0 }),
        InputActionDefinition::new(action::UI_MENU_RIGHT).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Menu right").with_effect(InputActionEffect::MenuNav { x: 1, y: 0 }),
    ]
}

#[inline]
pub fn gameplay_default_listeners() -> Vec<InputActionListenerRegistration> {
    vec![
        InputActionListenerRegistration::new("newengine-ui", "pause-menu")
            .with_actions([
                action::UI_MENU_TOGGLE,
                action::UI_MENU_ACCEPT,
                action::UI_MENU_BACK,
                action::UI_MENU_UP,
                action::UI_MENU_DOWN,
                action::UI_MENU_LEFT,
                action::UI_MENU_RIGHT,
            ])
            .with_priority(100)
            .consuming(),
        InputActionListenerRegistration::new("newengine-camera-runtime", "camera-view")
            .with_actions([
                action::CAMERA_VIEW_NEXT,
                action::CAMERA_VIEW_PREVIOUS,
                action::CAMERA_VIEW_FIRST_PERSON,
                action::CAMERA_VIEW_THIRD_PERSON_FOLLOW,
                action::CAMERA_VIEW_THIRD_PERSON_AIM,
            ])
            .with_priority(50),
        InputActionListenerRegistration::new("newengine-gameplay", "player-controller")
            .with_actions([
                action::PLAYER_MOVE_FORWARD,
                action::PLAYER_MOVE_BACK,
                action::PLAYER_MOVE_LEFT,
                action::PLAYER_MOVE_RIGHT,
                action::PLAYER_MOVE_UP,
                action::PLAYER_MOVE_DOWN,
                action::PLAYER_SPRINT,
            ])
            .with_priority(10),
    ]
}

fn ensure_required_system_bindings(bindings: &mut Vec<InputBinding>) {
    let has_keyboard_toggle = bindings.iter().any(|binding| {
        binding.action == action::UI_MENU_TOGGLE
            && binding.device == InputBindingDevice::Keyboard
            && binding.phase == InputBindingPhase::Pressed
    });
    if !has_keyboard_toggle {
        bindings.push(InputBinding::keyboard_pressed(action::UI_MENU_TOGGLE, key_code::ESCAPE));
    }

    let has_gamepad_toggle = bindings.iter().any(|binding| {
        binding.action == action::UI_MENU_TOGGLE
            && binding.device == InputBindingDevice::GamepadButton
            && binding.phase == InputBindingPhase::Pressed
    });
    if !has_gamepad_toggle {
        bindings.push(InputBinding::gamepad_button_pressed(action::UI_MENU_TOGGLE, gamepad_button::START));
    }

    bindings.retain(|binding| {
        !(binding.action == action::UI_MENU_BACK
            && binding.device == InputBindingDevice::Keyboard
            && binding.code == key_code::ESCAPE)
    });
}

#[inline]
pub fn gameplay_default_bindings() -> Vec<InputBinding> {
    let mut bindings = Vec::with_capacity(32);
    bindings.extend(gameplay_keyboard_bindings());
    bindings.extend(gameplay_gamepad_button_bindings());
    bindings
}

fn gameplay_keyboard_bindings() -> [InputBinding; 20] {
    use newengine_input_api::key_code as keys;
    [
        InputBinding::keyboard_down(action::PLAYER_MOVE_FORWARD, keys::KEY_W),
        InputBinding::keyboard_down(action::PLAYER_MOVE_BACK, keys::KEY_S),
        InputBinding::keyboard_down(action::PLAYER_MOVE_LEFT, keys::KEY_A),
        InputBinding::keyboard_down(action::PLAYER_MOVE_RIGHT, keys::KEY_D),
        InputBinding::keyboard_down(action::PLAYER_MOVE_UP, keys::KEY_Q),
        InputBinding::keyboard_down(action::PLAYER_MOVE_DOWN, keys::KEY_E),
        InputBinding::keyboard_down(action::PLAYER_SPRINT, keys::SHIFT_LEFT),
        InputBinding::keyboard_down(action::PLAYER_SPRINT, keys::SHIFT_RIGHT),
        InputBinding::keyboard_pressed(action::CAMERA_VIEW_NEXT, keys::KEY_F),
        InputBinding::keyboard_pressed(action::CAMERA_VIEW_FIRST_PERSON, keys::DIGIT1),
        InputBinding::keyboard_pressed(action::CAMERA_VIEW_THIRD_PERSON_FOLLOW, keys::DIGIT2),
        InputBinding::keyboard_pressed(action::CAMERA_VIEW_THIRD_PERSON_AIM, keys::DIGIT3),
        InputBinding::keyboard_pressed(action::UI_MENU_TOGGLE, keys::ESCAPE),
        InputBinding::keyboard_pressed(action::UI_MENU_ACCEPT, keys::ENTER),
        InputBinding::keyboard_pressed(action::UI_MENU_ACCEPT, keys::SPACE),
        InputBinding::keyboard_pressed(action::UI_MENU_BACK, keys::BACKSPACE),
        InputBinding::keyboard_pressed(action::UI_MENU_UP, keys::ARROW_UP),
        InputBinding::keyboard_pressed(action::UI_MENU_DOWN, keys::ARROW_DOWN),
        InputBinding::keyboard_pressed(action::UI_MENU_LEFT, keys::ARROW_LEFT),
        InputBinding::keyboard_pressed(action::UI_MENU_RIGHT, keys::ARROW_RIGHT),
    ]
}

fn gameplay_gamepad_button_bindings() -> [InputBinding; 13] {
    [
        InputBinding::gamepad_button_down(action::PLAYER_SPRINT, gamepad_button::LEFT_THUMB),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_NEXT, gamepad_button::SELECT),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_NEXT, gamepad_button::MODE),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_FIRST_PERSON, gamepad_button::DPAD_UP),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_THIRD_PERSON_FOLLOW, gamepad_button::DPAD_LEFT),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_THIRD_PERSON_AIM, gamepad_button::DPAD_RIGHT),
        InputBinding::gamepad_button_pressed(action::UI_MENU_TOGGLE, gamepad_button::START),
        InputBinding::gamepad_button_pressed(action::UI_MENU_ACCEPT, gamepad_button::SOUTH),
        InputBinding::gamepad_button_pressed(action::UI_MENU_BACK, gamepad_button::EAST),
        InputBinding::gamepad_button_pressed(action::UI_MENU_UP, gamepad_button::DPAD_UP),
        InputBinding::gamepad_button_pressed(action::UI_MENU_DOWN, gamepad_button::DPAD_DOWN),
        InputBinding::gamepad_button_pressed(action::UI_MENU_LEFT, gamepad_button::DPAD_LEFT),
        InputBinding::gamepad_button_pressed(action::UI_MENU_RIGHT, gamepad_button::DPAD_RIGHT),
    ]
}

#[inline]
pub fn gameplay_default_gamepad_axes() -> Vec<GamepadAxisBinding> {
    vec![
        GamepadAxisBinding::new(gamepad_axis::LEFT_STICK_X, GamepadAxisTarget::MoveX, 1.0),
        GamepadAxisBinding::new(gamepad_axis::LEFT_STICK_Y, GamepadAxisTarget::MoveZ, -1.0),
        GamepadAxisBinding::new(gamepad_axis::RIGHT_STICK_X, GamepadAxisTarget::LookX, 1.0),
        GamepadAxisBinding::new(gamepad_axis::RIGHT_STICK_Y, GamepadAxisTarget::LookY, -1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_has_camera_view_switching() {
        let profile = game_ready_input_profile();
        assert!(profile.keys.iter().any(|k| k.id == "keyboard.escape" && k.code == key_code::ESCAPE));
        assert!(profile.bindings.iter().any(|b| b.action == action::CAMERA_VIEW_NEXT));
        assert!(profile.bindings.iter().any(|b| b.action == action::PLAYER_MOVE_FORWARD));
        assert!(profile.actions.iter().any(|a| a.id == action::CAMERA_VIEW_NEXT));
        assert!(profile.listeners.iter().any(|l| l.id == "pause-menu"));
        assert!(profile.bindings.iter().any(|b| b.action == action::UI_MENU_TOGGLE && b.code == key_code::ESCAPE));
        assert!(!profile.bindings.iter().any(|b| b.action == action::UI_MENU_BACK && b.code == key_code::ESCAPE));
    }
}
