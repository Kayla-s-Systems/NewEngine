use super::*;

mod canonicalize;
mod mutate;
mod query;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputBindingsProfile {
    pub id: String,
    pub version: u32,
    #[serde(default)]
    pub device_preference: InputDevicePreference,
    #[serde(default)]
    pub keys: Vec<InputKeyRegistration>,
    #[serde(default)]
    pub actions: Vec<InputActionDefinition>,
    #[serde(default)]
    pub listeners: Vec<InputActionListenerRegistration>,
    #[serde(default)]
    pub bindings: Vec<InputBinding>,
    #[serde(default)]
    pub gamepad_axes: Vec<GamepadAxisBinding>,
}

impl InputBindingsProfile {
    #[inline]
    pub fn empty(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: 4,
            device_preference: InputDevicePreference::Hybrid,
            keys: Vec::new(),
            actions: Vec::new(),
            listeners: Vec::new(),
            bindings: Vec::new(),
            gamepad_axes: Vec::new(),
        }
    }
}

impl Default for InputBindingsProfile {
    #[inline]
    fn default() -> Self {
        Self::empty("newengine.input.profile")
    }
}
