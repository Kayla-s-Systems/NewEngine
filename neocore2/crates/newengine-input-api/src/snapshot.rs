use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputAxis2Snapshot {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputKeySnapshot {
    #[serde(default)]
    pub down: BTreeSet<u32>,
    #[serde(default)]
    pub pressed: BTreeSet<u32>,
    #[serde(default)]
    pub released: BTreeSet<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputMouseSnapshot {
    #[serde(default)]
    pub pos: InputAxis2Snapshot,
    #[serde(default)]
    pub delta: InputAxis2Snapshot,
    #[serde(default)]
    pub wheel: InputAxis2Snapshot,
    #[serde(default)]
    pub down: BTreeSet<u32>,
    #[serde(default)]
    pub pressed: BTreeSet<u32>,
    #[serde(default)]
    pub released: BTreeSet<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputTextSnapshot {
    #[serde(default)]
    pub buffer: String,
    #[serde(default)]
    pub ime_preedit: String,
    #[serde(default)]
    pub ime_commit: String,
    #[serde(default)]
    pub edit_ops: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputGamepadSnapshot {
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub buttons: BTreeMap<String, f32>,
    #[serde(default)]
    pub buttons_pressed: BTreeSet<String>,
    #[serde(default)]
    pub buttons_released: BTreeSet<String>,
    #[serde(default)]
    pub axes: BTreeMap<String, f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputDeviceSnapshot {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub connected: bool,
    #[serde(default, rename = "virtual")]
    pub virtual_device: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputStateSnapshot {
    #[serde(default)]
    pub keys: InputKeySnapshot,
    #[serde(default)]
    pub mouse: InputMouseSnapshot,
    #[serde(default)]
    pub text: InputTextSnapshot,
    #[serde(default)]
    pub gamepads: BTreeMap<String, InputGamepadSnapshot>,
    #[serde(default)]
    pub devices: BTreeMap<String, InputDeviceSnapshot>,
}
