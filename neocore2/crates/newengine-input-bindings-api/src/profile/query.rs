use super::*;

impl InputBindingsProfile {
    pub fn resolve<T: InputFrameSource>(&self, input: &T) -> InputActionFrame {
        let mut out = InputActionFrame::default();
        for binding in &self.bindings {
            if !binding_matches(binding, input)
                || out.actions.iter().any(|action| action == &binding.action)
            {
                continue;
            }
            let phase = match binding.phase {
                InputBindingPhase::Down => newengine_input_actions_api::InputActionPhase::Down,
                InputBindingPhase::Pressed => {
                    newengine_input_actions_api::InputActionPhase::Pressed
                }
                InputBindingPhase::Released => {
                    newengine_input_actions_api::InputActionPhase::Released
                }
            };
            if let Some(definition) = self
                .actions
                .iter()
                .find(|definition| definition.id == binding.action)
            {
                dispatch_action_definition(&mut out, definition, &self.listeners, phase);
            } else {
                out.actions.push(binding.action.clone());
                out.signals
                    .push(newengine_input_actions_api::InputActionSignal {
                        action: binding.action.clone(),
                        phase,
                    });
            }
        }
        if self.device_preference.allows_gamepad() {
            apply_gamepad_axes(&mut out, &self.gamepad_axes, input);
        }
        out
    }

    pub fn action_catalog(&self) -> std::collections::BTreeMap<&str, &InputActionDefinition> {
        self.actions
            .iter()
            .map(|definition| (definition.id.as_str(), definition))
            .collect()
    }

    #[inline]
    pub fn primary_binding_label(&self, action: &str) -> String {
        let action = newengine_input_actions_api::normalize_action_id(action)
            .unwrap_or_else(|| action.trim().to_owned());
        let preferred = match self.device_preference {
            InputDevicePreference::Gamepad => [
                InputBindingDevice::GamepadButton,
                InputBindingDevice::Keyboard,
                InputBindingDevice::MouseButton,
            ],
            InputDevicePreference::KeyboardMouse | InputDevicePreference::Hybrid => [
                InputBindingDevice::Keyboard,
                InputBindingDevice::MouseButton,
                InputBindingDevice::GamepadButton,
            ],
        };
        for device in preferred {
            if let Some(binding) = self
                .bindings
                .iter()
                .find(|binding| binding.action == action && binding.device == device)
            {
                return self.binding_display_label(binding);
            }
        }
        self.bindings
            .iter()
            .find(|binding| binding.action == action)
            .map(|binding| self.binding_display_label(binding))
            .unwrap_or_else(|| "UNBOUND".to_owned())
    }

    #[inline]
    pub fn key_label(&self, code: u32) -> String {
        self.keys
            .iter()
            .find(|key| key.code == code)
            .map(|key| key.label.clone())
            .unwrap_or_else(|| key_code_label(code).to_owned())
    }

    #[inline]
    pub fn binding_display_label(&self, binding: &InputBinding) -> String {
        match binding.device {
            InputBindingDevice::Keyboard => self.key_label(binding.code),
            InputBindingDevice::MouseButton => mouse_button_label(binding.code).to_owned(),
            InputBindingDevice::GamepadButton => binding
                .name
                .as_deref()
                .map(gamepad_button_label)
                .unwrap_or("GAMEPAD")
                .to_owned(),
        }
    }
}
