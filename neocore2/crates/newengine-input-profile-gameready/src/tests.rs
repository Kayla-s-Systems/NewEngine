use super::*;

#[test]
fn default_profile_has_camera_view_switching() {
    let profile = game_ready_input_profile();
    assert!(profile
        .keys
        .iter()
        .any(|k| k.id == "keyboard.escape" && k.code == engine_default_keybind::PRIMARY_UI_TOGGLE));
    assert!(profile
        .bindings
        .iter()
        .any(|b| b.action == action::CAMERA_VIEW_NEXT));
    assert!(profile
        .bindings
        .iter()
        .any(|b| b.action == action::PLAYER_MOVE_FORWARD));
    assert!(profile
        .actions
        .iter()
        .any(|a| a.id == action::CAMERA_VIEW_NEXT));
    assert!(profile.listeners.iter().any(|l| l.id == "ui-navigation"));
    assert!(profile.listeners.iter().any(|l| l.id == "asset-browser-ui"));
    assert!(profile
        .listeners
        .iter()
        .any(|l| l.id == "assets-browser-navigation"));
    assert!(profile
        .bindings
        .iter()
        .any(|b| b.action == action::UI_NAVIGATION_TOGGLE
            && b.code == engine_default_keybind::PRIMARY_UI_TOGGLE));
    assert!(profile
        .bindings
        .iter()
        .any(|b| b.action == action::ASSET_CATALOG_UI_TOGGLE
            && b.code == engine_default_keybind::ASSET_CATALOG_UI_TOGGLE));
    assert!(!profile
        .bindings
        .iter()
        .any(|b| b.action == action::UI_NAVIGATION_BACK
            && b.code == engine_default_keybind::PRIMARY_UI_TOGGLE));
}

#[test]
fn game_profile_reserves_numeric_hotkeys_for_equipment_and_i_for_inventory() {
    let profile = game_ready_game_input_profile();
    use newengine_input_api::key_code as keys;
    for (action, key) in [
        (action::EQUIP_PRIMARY, keys::DIGIT1),
        (action::EQUIP_SECONDARY, keys::DIGIT2),
        (action::EQUIP_SIDEARM, keys::DIGIT3),
        (action::EQUIP_MELEE, keys::DIGIT4),
        (action::EQUIP_THROWABLE, keys::DIGIT5),
        (action::INVENTORY_TOGGLE, keys::KEY_I),
    ] {
        assert!(profile.bindings.iter().any(|binding| {
            binding.action == action
                && binding.device == InputBindingDevice::Keyboard
                && binding.code == key
                && binding.phase == InputBindingPhase::Pressed
        }));
    }
    assert!(!profile.bindings.iter().any(|binding| {
        binding.device == InputBindingDevice::Keyboard
            && matches!(binding.code, keys::DIGIT1 | keys::DIGIT2 | keys::DIGIT3)
            && matches!(
                binding.action.as_str(),
                action::CAMERA_VIEW_FIRST_PERSON
                    | action::CAMERA_VIEW_THIRD_PERSON_FOLLOW
                    | action::CAMERA_VIEW_THIRD_PERSON_AIM
            )
    }));
    assert!(profile
        .listeners
        .iter()
        .any(|listener| listener.id == "inventory-controller"));
}

struct PressedKeyboardKey(u32);

impl newengine_input_actions_api::InputFrameSource for PressedKeyboardKey {
    fn is_key_down(&self, _key: u32) -> bool {
        false
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
fn game_profile_resolves_v_to_camera_view_cycle() {
    let profile = game_ready_game_input_profile();
    let frame = profile.resolve(&PressedKeyboardKey(newengine_input_api::key_code::KEY_V));

    assert!(frame.contains_action(action::CAMERA_VIEW_NEXT));
    assert_eq!(
        frame.camera_view,
        newengine_input_actions_api::CameraViewRequest::Next
    );
    assert!(profile.keys.iter().any(|key| {
        key.code == newengine_input_api::key_code::KEY_V
            && key.id == newengine_input_api::key_identity::KEY_V
    }));
    assert!(!profile.bindings.iter().any(|binding| {
        binding.action == action::CAMERA_VIEW_NEXT
            && binding.device == InputBindingDevice::Keyboard
            && binding.code == newengine_input_api::key_code::KEY_F
    }));
}

#[test]
fn game_profile_resolves_f1_to_hud_visibility_toggle() {
    let profile = game_ready_game_input_profile();
    let frame = profile.resolve(&PressedKeyboardKey(newengine_input_api::key_code::F1));

    assert!(frame.contains_action(action::HUD_VISIBILITY_TOGGLE));
    assert!(
        newengine_gameplay_fps_api::FpsActionFrame::from_commands(&frame.command_actions())
            .hud_visibility_toggle_pressed
    );
    assert!(!frame.contains_action(action::ASSET_CATALOG_UI_TOGGLE));
    assert!(profile.listeners.iter().any(|listener| {
        listener.id == "inventory-controller"
            && listener
                .action_filter
                .iter()
                .any(|action_id| action_id == action::HUD_VISIBILITY_TOGGLE)
    }));
}

#[test]
fn game_profile_resolves_m_to_playable_character_selector() {
    let profile = game_ready_game_input_profile();
    let frame = profile.resolve(&PressedKeyboardKey(newengine_input_api::key_code::KEY_M));

    assert!(frame.contains_action(action::CHARACTER_SELECT_TOGGLE));
    assert!(
        newengine_gameplay_fps_api::FpsActionFrame::from_commands(&frame.command_actions())
            .character_select_toggle_pressed
    );
    assert!(profile.keys.iter().any(|key| {
        key.code == newengine_input_api::key_code::KEY_M
            && key.id == newengine_input_api::key_identity::KEY_M
    }));
}

#[test]
fn game_profile_resolves_f7_to_noclip_toggle_and_keeps_vertical_flight_axes() {
    let profile = game_ready_game_input_profile();
    let frame = profile.resolve(&PressedKeyboardKey(newengine_input_api::key_code::F7));

    assert!(frame.contains_action(action::NOCLIP_TOGGLE));
    assert!(
        newengine_gameplay_fps_api::FpsActionFrame::from_commands(&frame.command_actions())
            .noclip_toggle_pressed
    );
    assert!(profile.keys.iter().any(|key| {
        key.code == newengine_input_api::key_code::F7
            && key.id == newengine_input_api::key_identity::F7
    }));
    assert!(profile.listeners.iter().any(|listener| {
        listener.id == "player-controller"
            && listener
                .action_filter
                .iter()
                .any(|id| id == action::NOCLIP_TOGGLE)
    }));
    for (action_id, key) in [
        (action::PLAYER_MOVE_UP, newengine_input_api::key_code::KEY_Q),
        (
            action::PLAYER_MOVE_DOWN,
            newengine_input_api::key_code::KEY_E,
        ),
    ] {
        assert!(profile.bindings.iter().any(|binding| {
            binding.action == action_id
                && binding.device == InputBindingDevice::Keyboard
                && binding.code == key
                && binding.phase == InputBindingPhase::Down
        }));
    }
}

#[test]
fn game_profile_excludes_editor_asset_browser() {
    let profile = game_ready_game_input_profile();
    assert!(profile.listeners.iter().any(|l| l.id == "ui-navigation"));
    assert!(!profile.listeners.iter().any(|l| l.id == "asset-browser-ui"));
    assert!(!profile
        .listeners
        .iter()
        .any(|l| l.id == "assets-browser-navigation"));
    assert!(!profile
        .actions
        .iter()
        .any(|a| a.id == action::ASSET_CATALOG_UI_TOGGLE));
    assert!(!profile
        .bindings
        .iter()
        .any(|b| b.action == action::ASSET_CATALOG_UI_TOGGLE));
    assert!(profile
        .bindings
        .iter()
        .any(|b| b.action == action::UI_NAVIGATION_TOGGLE
            && b.code == engine_default_keybind::PRIMARY_UI_TOGGLE));
    for action_id in [
        action::PLAYER_JUMP,
        action::PLAYER_CROUCH,
        action::PLAYER_FIRE_PRIMARY,
        action::PLAYER_AIM,
        action::PLAYER_RELOAD,
        action::PLAYER_INTERACT,
    ] {
        assert!(profile
            .actions
            .iter()
            .any(|definition| definition.id == action_id));
        assert!(profile
            .bindings
            .iter()
            .any(|binding| binding.action == action_id));
    }
    for action_id in [action::PLAYER_MOVE_UP, action::PLAYER_MOVE_DOWN] {
        assert!(profile
            .bindings
            .iter()
            .any(|binding| binding.action == action_id));
    }
    assert!(profile.bindings.iter().any(|binding| {
        binding.action == action::PLAYER_FIRE_PRIMARY
            && binding.device == InputBindingDevice::MouseButton
            && binding.code == mouse_button::LEFT
    }));
}

#[test]
fn game_profile_primary_fire_has_click_edge_without_demo_projectile_binding() {
    let profile = game_ready_game_input_profile();
    assert!(profile.bindings.iter().any(|binding| {
        binding.action == action::PLAYER_FIRE_PRIMARY
            && binding.device == InputBindingDevice::MouseButton
            && binding.code == mouse_button::LEFT
            && binding.phase == InputBindingPhase::Down
    }));
    assert!(profile.bindings.iter().any(|binding| {
        binding.action == action::PLAYER_FIRE_PRIMARY
            && binding.device == InputBindingDevice::MouseButton
            && binding.code == mouse_button::LEFT
            && binding.phase == InputBindingPhase::Pressed
    }));
    assert!(!profile.bindings.iter().any(|binding| {
        binding.action == action::PLAYER_LAUNCH_PROJECTILE
            && binding.device == InputBindingDevice::MouseButton
            && binding.code == mouse_button::LEFT
    }));
    assert!(
        !newengine_gameplay_fps_api::FpsGameplayPolicySnapshot::default()
            .player
            .allow_projectile_launch
    );
}

struct HeldMouseButton(u32);

impl newengine_input_actions_api::InputFrameSource for HeldMouseButton {
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
    fn is_mouse_pressed(&self, _button: u32) -> bool {
        false
    }
    fn is_mouse_released(&self, _button: u32) -> bool {
        false
    }
}

#[test]
fn game_profile_resolves_rmb_to_held_aim_command() {
    let profile = game_ready_game_input_profile();
    let frame = profile.resolve(&HeldMouseButton(newengine_input_api::mouse_button::RIGHT));
    assert!(frame.contains_action(action::PLAYER_AIM));
    let commands = frame.command_actions();
    assert!(commands.is_held(action::PLAYER_AIM));
    let fps = newengine_gameplay_fps_api::FpsActionFrame::from_commands(&commands);
    assert!(fps.aim_held);
}
