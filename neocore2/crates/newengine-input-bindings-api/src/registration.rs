use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputKeyRegistration {
    /// Stable canonical engine key code. Platform backends must explicitly map native keys to this value.
    pub code: u32,
    /// Stable semantic key id, e.g. `keyboard.escape` or `keyboard.key_w`.
    pub id: String,
    #[serde(default)]
    pub label: String,
}

impl InputKeyRegistration {
    pub fn new(code: u32, id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            code,
            id: id.into(),
            label: label.into(),
        }
    }

    pub fn normalized(mut self) -> Option<Self> {
        if self.code == 0 {
            return None;
        }
        self.id = newengine_input_actions_api::normalize_id_like(&self.id)?;
        self.label = self.label.trim().to_owned();
        if self.label.is_empty() {
            self.label = self.id.clone();
        }
        Some(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputBindingRegistration {
    pub binding: InputBinding,
    #[serde(default)]
    pub replace_existing_for_action_device: bool,
}

impl InputBindingRegistration {
    #[inline]
    pub fn new(binding: InputBinding) -> Self {
        Self {
            binding,
            replace_existing_for_action_device: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputBindingsManifest {
    #[serde(default)]
    pub keys: Vec<InputKeyRegistration>,
    #[serde(default)]
    pub actions: Vec<InputActionDefinition>,
    #[serde(default)]
    pub bindings: Vec<InputBindingRegistration>,
    #[serde(default)]
    pub listeners: Vec<InputActionListenerRegistration>,
    #[serde(default)]
    pub gamepad_axes: Vec<GamepadAxisBinding>,
}

impl InputBindingsManifest {
    pub fn apply_to(self, profile: &mut InputBindingsProfile) -> Result<(), String> {
        for key in self.keys {
            profile.register_key(key)?;
        }
        for action in self.actions {
            profile.register_action(action)?;
        }
        for binding in self.bindings {
            profile.register_binding(binding)?;
        }
        for listener in self.listeners {
            profile.register_listener(listener)?;
        }
        for axis in self.gamepad_axes {
            profile.register_gamepad_axis(axis)?;
        }
        Ok(())
    }
}
