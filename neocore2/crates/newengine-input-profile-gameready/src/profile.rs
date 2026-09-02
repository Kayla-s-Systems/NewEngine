use super::*;

#[inline]
pub fn game_ready_input_profile() -> InputBindingsProfile {
    let mut bindings = gameplay_default_bindings();
    ensure_required_system_bindings(&mut bindings);
    InputBindingsProfile {
        id: "newengine.gameready.input.profile".to_owned(),
        version: 8,
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
pub fn game_ready_game_input_profile() -> InputBindingsProfile {
    let mut profile = game_ready_input_profile();
    profile.id = "newengine.gameready.game.input.profile".to_owned();
    profile.version = profile.version.saturating_add(1);
    profile
        .actions
        .retain(|action| action.id != action::ASSET_CATALOG_UI_TOGGLE);
    profile.listeners.retain(|listener| {
        listener.id != "asset-browser-ui" && listener.id != "assets-browser-navigation"
    });
    profile.bindings.retain(|binding| {
        binding.action != action::ASSET_CATALOG_UI_TOGGLE
            && !(binding.device == InputBindingDevice::Keyboard
                && (newengine_input_api::key_code::DIGIT1..=newengine_input_api::key_code::DIGIT3)
                    .contains(&binding.code)
                && matches!(
                    binding.action.as_str(),
                    action::CAMERA_VIEW_FIRST_PERSON
                        | action::CAMERA_VIEW_THIRD_PERSON_FOLLOW
                        | action::CAMERA_VIEW_THIRD_PERSON_AIM
                ))
    });
    if !profile
        .actions
        .iter()
        .any(|definition| definition.id == action::HUD_VISIBILITY_TOGGLE)
    {
        profile.actions.push(
            InputActionDefinition::new(action::HUD_VISIBILITY_TOGGLE)
                .with_label("Toggle HUD visibility"),
        );
    }
    for listener in &mut profile.listeners {
        if listener.id == "inventory-controller"
            && !listener
                .action_filter
                .iter()
                .any(|action_id| action_id == action::HUD_VISIBILITY_TOGGLE)
        {
            listener
                .action_filter
                .push(action::HUD_VISIBILITY_TOGGLE.to_owned());
        }
    }
    profile.bindings.extend(standalone_fps_bindings());
    // In the playable game profile Space is authoritative jump input. Keeping the generic
    // UI-accept binding on the same physical key produces two cross-domain actions from one
    // edge and can hand gameplay movement to retained UI policy on the jump frame. Menus keep
    // Enter as keyboard accept; editor/general profiles retain their broader UI bindings.
    profile.bindings.retain(|binding| {
        !(binding.device == InputBindingDevice::Keyboard
            && binding.code == newengine_input_api::key_code::SPACE
            && binding.action == action::UI_NAVIGATION_ACCEPT)
            && !(binding.device == InputBindingDevice::Keyboard
                && binding.code == newengine_input_api::key_code::F1
                && binding.action != action::HUD_VISIBILITY_TOGGLE)
    });
    profile.bindings.push(InputBinding::keyboard_pressed(
        action::HUD_VISIBILITY_TOGGLE,
        newengine_input_api::key_code::F1,
    ));
    profile.canonicalized()
}
