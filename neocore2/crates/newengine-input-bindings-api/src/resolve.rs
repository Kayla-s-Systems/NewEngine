use super::*;

#[inline]
pub(crate) fn bindings_equivalent(a: &InputBinding, b: &InputBinding) -> bool {
    a.action == b.action
        && a.device == b.device
        && a.code == b.code
        && a.name == b.name
        && a.phase == b.phase
}

pub(crate) fn canonical_axis_binding(mut axis: GamepadAxisBinding) -> Option<GamepadAxisBinding> {
    axis.axis = axis.axis.trim().to_owned();
    axis.deadzone = axis.deadzone.clamp(0.0, 0.95);
    axis.scale = axis.scale.clamp(-8.0, 8.0);
    if axis.axis.is_empty() {
        None
    } else {
        Some(axis)
    }
}

pub(crate) fn upsert_key_registration(
    keys: &mut Vec<InputKeyRegistration>,
    key: InputKeyRegistration,
) {
    keys.retain(|existing| existing.code != key.code && existing.id != key.id);
    keys.push(key);
}

pub(crate) fn upsert_action_definition(
    actions: &mut Vec<InputActionDefinition>,
    action: InputActionDefinition,
) {
    if let Some(slot) = actions.iter_mut().find(|existing| existing.id == action.id) {
        *slot = action;
    } else {
        actions.push(action);
    }
}

pub(crate) fn upsert_listener_registration(
    listeners: &mut Vec<InputActionListenerRegistration>,
    listener: InputActionListenerRegistration,
) {
    listeners.retain(|existing| !(existing.owner == listener.owner && existing.id == listener.id));
    listeners.push(listener);
}

#[inline]
pub fn input_device_preference_is_display_only(_preference: InputDevicePreference) -> bool {
    // Device preference is intentionally not a hard gameplay input gate. It orders binding
    // labels and menu presentation only. Exclusive capture belongs to `engine.input.contexts`,
    // where modal policy can be expressed explicitly instead of silently disabling fallback
    // devices at the action resolver level.
    true
}

#[inline]
pub(crate) fn binding_matches<T: InputFrameSource>(binding: &InputBinding, input: &T) -> bool {
    match binding.device {
        InputBindingDevice::Keyboard => match binding.phase {
            InputBindingPhase::Down => input.is_key_down(binding.code),
            InputBindingPhase::Pressed => input.is_key_pressed(binding.code),
            InputBindingPhase::Released => input.is_key_released(binding.code),
        },
        InputBindingDevice::MouseButton => match binding.phase {
            InputBindingPhase::Down => input.is_mouse_down(binding.code),
            InputBindingPhase::Pressed => input.is_mouse_pressed(binding.code),
            InputBindingPhase::Released => input.is_mouse_released(binding.code),
        },
        InputBindingDevice::GamepadButton => {
            let Some(name) = binding.name.as_deref() else {
                return false;
            };
            match binding.phase {
                InputBindingPhase::Down => input.is_gamepad_button_down(name),
                InputBindingPhase::Pressed => input.is_gamepad_button_pressed(name),
                InputBindingPhase::Released => input.is_gamepad_button_released(name),
            }
        }
    }
}

pub(crate) fn apply_gamepad_axes<T: InputFrameSource>(
    out: &mut InputActionFrame,
    axes: &[GamepadAxisBinding],
    input: &T,
) {
    for axis in axes {
        let mut value = input.gamepad_axis(&axis.axis);
        if value.abs() < axis.deadzone {
            value = 0.0;
        }
        value = (value * axis.scale).clamp(-1.0, 1.0);
        match axis.target {
            GamepadAxisTarget::MoveX => out.move_axis[0] += value,
            GamepadAxisTarget::MoveY => out.move_axis[1] += value,
            GamepadAxisTarget::MoveZ => out.move_axis[2] += value,
            GamepadAxisTarget::LookX => out.look_axis[0] += value,
            GamepadAxisTarget::LookY => out.look_axis[1] += value,
        }
    }
    out.move_axis = out.move_axis.map(|v| v.clamp(-1.0, 1.0));
    out.look_axis = out.look_axis.map(|v| v.clamp(-1.0, 1.0));
    if out.move_axis[0] > 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::RIGHT;
    }
    if out.move_axis[0] < 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::LEFT;
    }
    if out.move_axis[1] > 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::UP;
    }
    if out.move_axis[1] < 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::DOWN;
    }
    if out.move_axis[2] > 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::FORWARD;
    }
    if out.move_axis[2] < 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::BACK;
    }
}

pub(crate) fn dispatch_action_definition(
    out: &mut InputActionFrame,
    definition: &InputActionDefinition,
    listeners: &[InputActionListenerRegistration],
) {
    out.actions.push(definition.id.clone());
    out.events.push(dispatch_event_for(definition, listeners));
    for effect in &definition.effects {
        apply_action_effect(out, effect);
    }
    out.move_axis = newengine_input_actions_api::move_axis_from_mask(out.move_mask);
}

fn dispatch_event_for(
    definition: &InputActionDefinition,
    listeners: &[InputActionListenerRegistration],
) -> newengine_input_actions_api::InputActionDispatchEvent {
    let mut event = newengine_input_actions_api::InputActionDispatchEvent {
        action: definition.id.clone(),
        listeners: Vec::new(),
        consumed_by: None,
    };
    for listener in listeners
        .iter()
        .filter(|listener| listener.enabled && listener_matches_action(listener, definition))
    {
        let listener_id = format!("{}:{}", listener.owner, listener.id);
        event.listeners.push(listener_id.clone());
        if definition.dispatch == InputActionDispatchMode::ConsumeFirst && listener.consume {
            event.consumed_by = Some(listener_id);
            break;
        }
    }
    event
}

fn listener_matches_action(
    listener: &InputActionListenerRegistration,
    definition: &InputActionDefinition,
) -> bool {
    let action_match = listener.action_filter.is_empty()
        || listener
            .action_filter
            .iter()
            .any(|action| action == &definition.id);
    if !action_match {
        return false;
    }
    listener.context_filter.is_empty()
        || definition
            .contexts
            .iter()
            .any(|ctx| listener.context_filter.iter().any(|wanted| wanted == ctx))
}

fn apply_action_effect(out: &mut InputActionFrame, effect: &InputActionEffect) {
    match effect {
        InputActionEffect::MoveMask { mask } => out.move_mask |= *mask,
        InputActionEffect::Sprint { enabled } => out.sprint |= *enabled,
        InputActionEffect::CameraView { request } => out.camera_view = *request,
        InputActionEffect::UiToggle => out.ui_toggle = true,
        InputActionEffect::UiAccept => out.ui_accept = true,
        InputActionEffect::UiBack => out.ui_back = true,
        InputActionEffect::UiNav { x, y } => {
            out.ui_nav[0] = out.ui_nav[0].saturating_add(*x).clamp(-1, 1);
            out.ui_nav[1] = out.ui_nav[1].saturating_add(*y).clamp(-1, 1);
        }
    }
}
