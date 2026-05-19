#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub const ENGINE_INPUT_SERVICE_ID: &str = "engine.input";
pub const INPUT_SERVICE_ID: &str = "newengine.input.v1";
pub const INPUT_BACKEND_CAPABILITY_ID: &str = "input.backend";

pub const INPUT_METHOD_INGEST_JSON: &str = "ingest_json";
pub const INPUT_METHOD_STATE_JSON: &str = "state_json";
pub const INPUT_METHOD_TEXT_TAKE_JSON: &str = "text_take_json";
pub const INPUT_METHOD_IME_COMMIT_TAKE_JSON: &str = "ime_commit_take_json";

pub const INPUT_REQUIRED_METHODS: &[&str] = &[
    INPUT_METHOD_STATE_JSON,
    INPUT_METHOD_TEXT_TAKE_JSON,
    INPUT_METHOD_IME_COMMIT_TAKE_JSON,
];

pub const INPUT_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "input",
        ENGINE_INPUT_SERVICE_ID,
        INPUT_SERVICE_ID,
        INPUT_BACKEND_CAPABILITY_ID,
    );

pub const INPUT_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_INPUT_SERVICE_ID,
        "newengine.input service >= 0.3.x",
        INPUT_REQUIRED_METHODS,
    );

pub const INPUT_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        INPUT_RUNTIME_CONTRACT_SPEC,
        Some(INPUT_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_INPUT_BACKEND"),
    );



/// Canonical engine key codes used by raw input providers and binding profiles.
/// Platform plugins must explicitly translate native key enums into these stable values.
pub mod key_code {
    pub const DIGIT1: u32 = 6;
    pub const DIGIT2: u32 = 7;
    pub const DIGIT3: u32 = 8;

    pub const KEY_A: u32 = 19;
    pub const KEY_D: u32 = 22;
    pub const KEY_E: u32 = 23;
    pub const KEY_F: u32 = 24;
    pub const KEY_Q: u32 = 35;
    pub const KEY_S: u32 = 37;
    pub const KEY_W: u32 = 41;

    pub const ENTER: u32 = 57;
    pub const SPACE: u32 = 58;
    pub const SHIFT_LEFT: u32 = 60;
    pub const SHIFT_RIGHT: u32 = 61;
    pub const TAB: u32 = 63;
    pub const BACKSPACE: u32 = 70;

    pub const ARROW_LEFT: u32 = 86;
    pub const ARROW_UP: u32 = 87;
    pub const ARROW_RIGHT: u32 = 88;
    pub const ARROW_DOWN: u32 = 89;

    pub const ESCAPE: u32 = 114;
}

/// Canonical gamepad button names used by raw input providers and binding profiles.
pub mod gamepad_button {
    pub const SOUTH: &str = "South";
    pub const EAST: &str = "East";
    pub const WEST: &str = "West";
    pub const NORTH: &str = "North";
    pub const LEFT_TRIGGER: &str = "LeftTrigger";
    pub const LEFT_TRIGGER_2: &str = "LeftTrigger2";
    pub const RIGHT_TRIGGER: &str = "RightTrigger";
    pub const RIGHT_TRIGGER_2: &str = "RightTrigger2";
    pub const SELECT: &str = "Select";
    pub const START: &str = "Start";
    pub const MODE: &str = "Mode";
    pub const LEFT_THUMB: &str = "LeftThumb";
    pub const RIGHT_THUMB: &str = "RightThumb";
    pub const DPAD_UP: &str = "DPadUp";
    pub const DPAD_DOWN: &str = "DPadDown";
    pub const DPAD_LEFT: &str = "DPadLeft";
    pub const DPAD_RIGHT: &str = "DPadRight";
}

/// Canonical gamepad axis names used by raw input providers and binding profiles.
pub mod gamepad_axis {
    pub const LEFT_STICK_X: &str = "LeftStickX";
    pub const LEFT_STICK_Y: &str = "LeftStickY";
    pub const RIGHT_STICK_X: &str = "RightStickX";
    pub const RIGHT_STICK_Y: &str = "RightStickY";
    pub const LEFT_Z: &str = "LeftZ";
    pub const RIGHT_Z: &str = "RightZ";
}

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
