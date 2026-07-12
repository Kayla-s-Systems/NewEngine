use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputDevicePreference {
    KeyboardMouse,
    Gamepad,
    Hybrid,
}

impl Default for InputDevicePreference {
    #[inline]
    fn default() -> Self {
        Self::Hybrid
    }
}

impl InputDevicePreference {
    #[inline]
    pub fn allows_keyboard_mouse(self) -> bool {
        matches!(self, Self::KeyboardMouse | Self::Hybrid)
    }

    #[inline]
    pub fn allows_gamepad(self) -> bool {
        matches!(self, Self::Gamepad | Self::Hybrid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputBindingPhase {
    Down,
    Pressed,
    Released,
}

impl Default for InputBindingPhase {
    #[inline]
    fn default() -> Self {
        Self::Down
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputBindingDevice {
    Keyboard,
    MouseButton,
    GamepadButton,
}

impl Default for InputBindingDevice {
    #[inline]
    fn default() -> Self {
        Self::Keyboard
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputBinding {
    pub action: String,
    #[serde(default)]
    pub device: InputBindingDevice,
    /// Numeric code for keyboard/mouse bindings.
    #[serde(default)]
    pub code: u32,
    /// Stable symbolic name for gamepad bindings, e.g. `South`, `Start`, `DPadUp`.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub phase: InputBindingPhase,
}

impl InputBinding {
    #[inline]
    pub fn normalized(mut self) -> Option<Self> {
        self.action = newengine_input_actions_api::normalize_action_id(&self.action)?;
        if let Some(name) = self.name.take() {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                self.name = Some(trimmed.to_owned());
            }
        }
        Some(self)
    }

    #[inline]
    pub fn keyboard_down(action: impl Into<String>, code: u32) -> Self {
        Self {
            action: action.into(),
            device: InputBindingDevice::Keyboard,
            code,
            name: None,
            phase: InputBindingPhase::Down,
        }
    }

    #[inline]
    pub fn keyboard_pressed(action: impl Into<String>, code: u32) -> Self {
        Self {
            action: action.into(),
            device: InputBindingDevice::Keyboard,
            code,
            name: None,
            phase: InputBindingPhase::Pressed,
        }
    }

    #[inline]
    pub fn mouse_button_down(action: impl Into<String>, code: u32) -> Self {
        Self {
            action: action.into(),
            device: InputBindingDevice::MouseButton,
            code,
            name: None,
            phase: InputBindingPhase::Down,
        }
    }

    #[inline]
    pub fn mouse_button_pressed(action: impl Into<String>, code: u32) -> Self {
        Self {
            action: action.into(),
            device: InputBindingDevice::MouseButton,
            code,
            name: None,
            phase: InputBindingPhase::Pressed,
        }
    }

    #[inline]
    pub fn gamepad_button_down(action: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            device: InputBindingDevice::GamepadButton,
            code: 0,
            name: Some(name.into()),
            phase: InputBindingPhase::Down,
        }
    }

    #[inline]
    pub fn gamepad_button_pressed(action: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            device: InputBindingDevice::GamepadButton,
            code: 0,
            name: Some(name.into()),
            phase: InputBindingPhase::Pressed,
        }
    }
}
