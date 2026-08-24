use super::*;

pub(crate) fn action_payload_f32(
    action: &UiActionDispatch,
    field: &str,
    index: usize,
) -> Option<f32> {
    action_payload_array_f32(action, field, index)
}

pub(crate) fn action_payload_array_f32(
    action: &UiActionDispatch,
    field: &str,
    index: usize,
) -> Option<f32> {
    action
        .payload
        .get(field)?
        .as_array()?
        .get(index)?
        .as_f64()
        .map(|value| value as f32)
}

pub(crate) fn dispatch_wheel_y(dispatch: &UiEventDispatchFrame, surface_id: &str) -> Option<f32> {
    dispatch
        .actions
        .iter()
        .find(|action| {
            action.surface_id == surface_id
                && action.trigger == UiNodeEventTrigger::ValueChanged
                && action.action_id == UI_SCROLL_WHEEL_ACTION
        })
        .and_then(|action| action_payload_array_f32(action, "wheel", 1))
}

pub(crate) fn breadcrumb_path_from_action(
    action: &UiActionDispatch,
    snapshot: &AssetsCatalogSnapshot,
) -> String {
    let Some(local_x) = action_payload_f32(action, "local_pos", 0) else {
        return parent_path(&snapshot.logical_path);
    };
    let Some(width) = action_payload_f32(action, "global_rect", 2).filter(|width| *width > 0.0)
    else {
        return parent_path(&snapshot.logical_path);
    };
    hit_breadcrumb_path(snapshot, local_x, 0.0, width)
}

pub(crate) fn editing_tools_available(resources: &Resources) -> bool {
    resources
        .get::<newengine_plugin_host::PluginsSnapshot>()
        .is_some_and(|snapshot| {
            snapshot.has_running_capability(newengine_plugin_api::CAPABILITY_ID_EDITING_TOOLS)
        })
}

pub(crate) fn set_input_capture_contribution(
    resources: &mut Resources,
    owner: &str,
    capture: UiInputCaptureState,
) {
    let mut manager = resources
        .remove::<UiInputCaptureStateManager>()
        .unwrap_or_default();
    manager.add_capture(owner.to_owned(), capture);
    let resolved = manager.resolve_final_capture();
    resources.insert(manager);
    resources.insert(resolved);
}

pub(crate) fn remove_input_capture_contribution(
    resources: &mut Resources,
    owner: &str,
    refresh_surface: Option<&str>,
) {
    let mut manager = resources
        .remove::<UiInputCaptureStateManager>()
        .unwrap_or_default();
    manager.remove_capture(owner);
    let mut resolved = manager.resolve_final_capture();
    if let Some(surface) = refresh_surface {
        resolved.draw_refresh_requested = true;
        if !resolved.surfaces.iter().any(|it| it == surface) {
            resolved.surfaces.push(surface.to_owned());
        }
    }
    resources.insert(manager);
    resources.insert(resolved);
}

pub(crate) struct UiInputSource<'a>(&'a UiInputFrame);

impl InputFrameSource for UiInputSource<'_> {
    #[inline]
    fn is_key_down(&self, key: u32) -> bool {
        self.0.keys_down.contains(&key)
    }
    #[inline]
    fn is_key_pressed(&self, key: u32) -> bool {
        self.0.keys_pressed.contains(&key)
    }
    #[inline]
    fn is_key_released(&self, key: u32) -> bool {
        self.0.keys_released.contains(&key)
    }
    #[inline]
    fn is_mouse_down(&self, button: u32) -> bool {
        self.0.mouse_down.contains(&button)
    }
    #[inline]
    fn is_mouse_pressed(&self, button: u32) -> bool {
        self.0.mouse_pressed.contains(&button)
    }
    #[inline]
    fn is_mouse_released(&self, button: u32) -> bool {
        self.0.mouse_released.contains(&button)
    }
    #[inline]
    fn has_gamepad_connected(&self) -> bool {
        self.0.gamepad_connected > 0
    }
    #[inline]
    fn is_gamepad_button_down(&self, button: &str) -> bool {
        self.0.is_gamepad_button_down(button)
    }
    #[inline]
    fn is_gamepad_button_pressed(&self, button: &str) -> bool {
        self.0.gamepad_buttons_pressed.contains(button)
    }
    #[inline]
    fn is_gamepad_button_released(&self, button: &str) -> bool {
        self.0.gamepad_buttons_released.contains(button)
    }
    #[inline]
    fn gamepad_axis(&self, axis: &str) -> f32 {
        self.0.gamepad_axes.get(axis).copied().unwrap_or(0.0)
    }
}

pub(crate) fn resolve_actions(input: &UiInputFrame) -> InputActionFrame {
    newengine_input_bindings_runtime::resolve_input_actions(&UiInputSource(input))
}

pub(crate) fn action_frame_contains(actions: &InputActionFrame, action: &str) -> bool {
    actions.actions.iter().any(|it| it == action)
        || actions.events.iter().any(|event| event.action == action)
}

pub(crate) fn ensure_assets_catalog_input_registration() -> bool {
    let mut ok = true;
    for (code, identity, label) in [
        (
            engine_default_keybind::ASSET_CATALOG_UI_TOGGLE,
            key_identity::F1,
            "F1",
        ),
        (key_code::ARROW_UP, key_identity::ARROW_UP, "Arrow Up"),
        (key_code::ARROW_DOWN, key_identity::ARROW_DOWN, "Arrow Down"),
        (key_code::ARROW_LEFT, key_identity::ARROW_LEFT, "Arrow Left"),
        (
            key_code::ARROW_RIGHT,
            key_identity::ARROW_RIGHT,
            "Arrow Right",
        ),
        (key_code::ENTER, key_identity::ENTER, "Enter"),
        (key_code::BACKSPACE, key_identity::BACKSPACE, "Backspace"),
    ] {
        if let Err(error) = newengine_input_bindings_runtime::register_input_key(
            InputKeyRegistration::new(code, identity, label),
        ) {
            newengine_ulog_api::ulog::warn!(
                "asset browser UI: key registration failed key='{label}': {error}"
            );
            ok = false;
        }
    }

    for action in [
        InputActionDefinition::new(engine_action::ASSET_CATALOG_UI_TOGGLE)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Toggle Asset Browser"),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_ACCEPT)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog accept")
            .with_effect(InputActionEffect::UiAccept),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_BACK)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog back")
            .with_effect(InputActionEffect::UiBack),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_UP)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog up")
            .with_effect(InputActionEffect::UiNav { x: 0, y: -1 }),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_DOWN)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog down")
            .with_effect(InputActionEffect::UiNav { x: 0, y: 1 }),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_LEFT)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog previous view")
            .with_effect(InputActionEffect::UiNav { x: -1, y: 0 }),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_RIGHT)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog next view")
            .with_effect(InputActionEffect::UiNav { x: 1, y: 0 }),
    ] {
        if let Err(error) = newengine_input_bindings_runtime::register_input_action(action) {
            newengine_ulog_api::ulog::warn!(
                "asset browser UI: action registration failed: {error}"
            );
            ok = false;
        }
    }

    for registration in [
        InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::ASSET_CATALOG_UI_TOGGLE,
            engine_default_keybind::ASSET_CATALOG_UI_TOGGLE,
        )),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::UI_NAVIGATION_ACCEPT,
            key_code::ENTER,
        )),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::UI_NAVIGATION_BACK,
            key_code::BACKSPACE,
        )),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::UI_NAVIGATION_UP,
            key_code::ARROW_UP,
        )),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::UI_NAVIGATION_DOWN,
            key_code::ARROW_DOWN,
        )),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::UI_NAVIGATION_LEFT,
            key_code::ARROW_LEFT,
        )),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::UI_NAVIGATION_RIGHT,
            key_code::ARROW_RIGHT,
        )),
    ] {
        if let Err(error) = newengine_input_bindings_runtime::register_input_binding(registration) {
            newengine_ulog_api::ulog::warn!(
                "asset browser UI: binding registration failed: {error}"
            );
            ok = false;
        }
    }

    if let Err(error) = newengine_input_bindings_runtime::register_input_listener(
        newengine_input_actions_api::InputActionListenerRegistration::new(
            ASSETS_CATALOG_UI_OWNER,
            ASSETS_CATALOG_INPUT_LISTENER,
        )
        .with_actions([engine_action::ASSET_CATALOG_UI_TOGGLE])
        .with_priority(110)
        .consuming(),
    ) {
        newengine_ulog_api::ulog::warn!(
            "asset browser UI: toggle listener registration failed: {error}"
        );
        ok = false;
    }

    if let Err(error) = newengine_input_bindings_runtime::register_input_listener(
        newengine_input_actions_api::InputActionListenerRegistration::new(
            ASSETS_CATALOG_UI_OWNER,
            "assets-browser-navigation",
        )
        .with_actions([
            engine_action::UI_NAVIGATION_ACCEPT,
            engine_action::UI_NAVIGATION_BACK,
            engine_action::UI_NAVIGATION_UP,
            engine_action::UI_NAVIGATION_DOWN,
            engine_action::UI_NAVIGATION_LEFT,
            engine_action::UI_NAVIGATION_RIGHT,
        ])
        .with_priority(110),
    ) {
        newengine_ulog_api::ulog::warn!(
            "asset browser UI: navigation listener registration failed: {error}"
        );
        ok = false;
    }

    if ok {
        newengine_ulog_api::ulog::info!(
            "asset browser UI: input listeners registered owner='{}' toggle_listener='{}' nav_listener='assets-browser-navigation'",
            ASSETS_CATALOG_UI_OWNER,
            ASSETS_CATALOG_INPUT_LISTENER,
        );
    }
    ok
}
