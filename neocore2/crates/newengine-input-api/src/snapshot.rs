use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputGamepadSnapshot {
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub buttons: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub buttons_pressed: Vec<String>,
    #[serde(default)]
    pub buttons_released: Vec<String>,
    #[serde(default)]
    pub axes: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputStateSnapshot {
    #[serde(default)]
    pub gamepads: serde_json::Map<String, serde_json::Value>,
}
