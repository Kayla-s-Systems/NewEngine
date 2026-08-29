use super::*;

pub(crate) fn ensure_required_system_bindings(bindings: &mut Vec<InputBinding>) {
    let has_keyboard_toggle = bindings.iter().any(|binding| {
        binding.action == action::UI_NAVIGATION_TOGGLE
            && binding.device == InputBindingDevice::Keyboard
            && binding.phase == InputBindingPhase::Pressed
    });
    if !has_keyboard_toggle {
        bindings.push(InputBinding::keyboard_pressed(
            action::UI_NAVIGATION_TOGGLE,
            engine_default_keybind::PRIMARY_UI_TOGGLE,
        ));
    }

    let has_gamepad_toggle = bindings.iter().any(|binding| {
        binding.action == action::UI_NAVIGATION_TOGGLE
            && binding.device == InputBindingDevice::GamepadButton
            && binding.phase == InputBindingPhase::Pressed
    });
    if !has_gamepad_toggle {
        bindings.push(InputBinding::gamepad_button_pressed(
            action::UI_NAVIGATION_TOGGLE,
            gamepad_button::START,
        ));
    }

    let has_asset_catalog_toggle = bindings.iter().any(|binding| {
        binding.action == action::ASSET_CATALOG_UI_TOGGLE
            && binding.device == InputBindingDevice::Keyboard
            && binding.phase == InputBindingPhase::Pressed
    });
    if !has_asset_catalog_toggle {
        bindings.push(InputBinding::keyboard_pressed(
            action::ASSET_CATALOG_UI_TOGGLE,
            engine_default_keybind::ASSET_CATALOG_UI_TOGGLE,
        ));
    }

    bindings.retain(|binding| {
        !(binding.action == action::UI_NAVIGATION_BACK
            && binding.device == InputBindingDevice::Keyboard
            && binding.code == engine_default_keybind::PRIMARY_UI_TOGGLE)
    });
}

#[inline]
pub fn gameplay_default_bindings() -> Vec<InputBinding> {
    let mut bindings = Vec::with_capacity(32);
    bindings.extend(gameplay_keyboard_bindings());
    bindings.extend(gameplay_gamepad_button_bindings());
    bindings
}

pub(crate) fn standalone_fps_bindings() -> Vec<InputBinding> {
    use newengine_input_api::key_code as keys;
    vec![
        InputBinding::keyboard_pressed(action::PLAYER_JUMP, keys::SPACE),
        InputBinding::keyboard_down(action::PLAYER_CROUCH, keys::KEY_C),
        InputBinding::keyboard_down(action::PLAYER_CROUCH, keys::CONTROL_LEFT),
        InputBinding::keyboard_down(action::PLAYER_CROUCH, keys::CONTROL_RIGHT),
        InputBinding::keyboard_pressed(action::PLAYER_RELOAD, keys::KEY_R),
        InputBinding::keyboard_pressed(action::PLAYER_INTERACT, keys::KEY_E),
        InputBinding::keyboard_pressed(action::INVENTORY_TOGGLE, keys::KEY_I),
        // Character selector is a real keyboard modal. Keep explicit press/release
        // phases so M can open, release can re-arm, and the next M press can close
        // without an arbitrary multi-frame timeout.
        InputBinding::keyboard_pressed(action::CHARACTER_SELECT_TOGGLE, keys::KEY_M),
        InputBinding::keyboard_released(action::CHARACTER_SELECT_TOGGLE, keys::KEY_M),
        InputBinding::keyboard_pressed(action::EQUIP_PRIMARY, keys::DIGIT1),
        InputBinding::keyboard_pressed(action::EQUIP_SECONDARY, keys::DIGIT2),
        InputBinding::keyboard_pressed(action::EQUIP_SIDEARM, keys::DIGIT3),
        InputBinding::keyboard_pressed(action::EQUIP_MELEE, keys::DIGIT4),
        InputBinding::keyboard_pressed(action::EQUIP_THROWABLE, keys::DIGIT5),
        InputBinding::mouse_button_down(action::PLAYER_FIRE_PRIMARY, mouse_button::LEFT),
        InputBinding::mouse_button_pressed(action::PLAYER_FIRE_PRIMARY, mouse_button::LEFT),
        InputBinding::mouse_button_down(action::PLAYER_AIM, mouse_button::RIGHT),
        InputBinding::gamepad_button_pressed(action::PLAYER_JUMP, gamepad_button::SOUTH),
        InputBinding::gamepad_button_down(action::PLAYER_CROUCH, gamepad_button::RIGHT_THUMB),
        InputBinding::gamepad_button_down(
            action::PLAYER_FIRE_PRIMARY,
            gamepad_button::RIGHT_TRIGGER_2,
        ),
        InputBinding::gamepad_button_pressed(
            action::PLAYER_FIRE_PRIMARY,
            gamepad_button::RIGHT_TRIGGER_2,
        ),
        InputBinding::gamepad_button_down(action::PLAYER_AIM, gamepad_button::LEFT_TRIGGER_2),
        InputBinding::gamepad_button_pressed(action::PLAYER_RELOAD, gamepad_button::WEST),
        InputBinding::gamepad_button_pressed(action::PLAYER_INTERACT, gamepad_button::NORTH),
    ]
}

fn gameplay_keyboard_bindings() -> [InputBinding; 19] {
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
        InputBinding::keyboard_pressed(action::CAMERA_VIEW_NEXT, keys::KEY_V),
        InputBinding::keyboard_pressed(action::CAMERA_VIEW_FIRST_PERSON, keys::DIGIT1),
        InputBinding::keyboard_pressed(action::CAMERA_VIEW_THIRD_PERSON_FOLLOW, keys::DIGIT2),
        InputBinding::keyboard_pressed(action::CAMERA_VIEW_THIRD_PERSON_AIM, keys::DIGIT3),
        InputBinding::keyboard_pressed(action::UI_NAVIGATION_ACCEPT, keys::ENTER),
        InputBinding::keyboard_pressed(action::UI_NAVIGATION_ACCEPT, keys::SPACE),
        InputBinding::keyboard_pressed(action::UI_NAVIGATION_BACK, keys::BACKSPACE),
        InputBinding::keyboard_pressed(action::UI_NAVIGATION_UP, keys::ARROW_UP),
        InputBinding::keyboard_pressed(action::UI_NAVIGATION_DOWN, keys::ARROW_DOWN),
        InputBinding::keyboard_pressed(action::UI_NAVIGATION_LEFT, keys::ARROW_LEFT),
        InputBinding::keyboard_pressed(action::UI_NAVIGATION_RIGHT, keys::ARROW_RIGHT),
    ]
}

fn gameplay_gamepad_button_bindings() -> [InputBinding; 12] {
    [
        InputBinding::gamepad_button_down(action::PLAYER_SPRINT, gamepad_button::LEFT_THUMB),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_NEXT, gamepad_button::SELECT),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_NEXT, gamepad_button::MODE),
        InputBinding::gamepad_button_pressed(
            action::CAMERA_VIEW_FIRST_PERSON,
            gamepad_button::DPAD_UP,
        ),
        InputBinding::gamepad_button_pressed(
            action::CAMERA_VIEW_THIRD_PERSON_FOLLOW,
            gamepad_button::DPAD_LEFT,
        ),
        InputBinding::gamepad_button_pressed(
            action::CAMERA_VIEW_THIRD_PERSON_AIM,
            gamepad_button::DPAD_RIGHT,
        ),
        InputBinding::gamepad_button_pressed(action::UI_NAVIGATION_ACCEPT, gamepad_button::SOUTH),
        InputBinding::gamepad_button_pressed(action::UI_NAVIGATION_BACK, gamepad_button::EAST),
        InputBinding::gamepad_button_pressed(action::UI_NAVIGATION_UP, gamepad_button::DPAD_UP),
        InputBinding::gamepad_button_pressed(action::UI_NAVIGATION_DOWN, gamepad_button::DPAD_DOWN),
        InputBinding::gamepad_button_pressed(action::UI_NAVIGATION_LEFT, gamepad_button::DPAD_LEFT),
        InputBinding::gamepad_button_pressed(
            action::UI_NAVIGATION_RIGHT,
            gamepad_button::DPAD_RIGHT,
        ),
    ]
}
