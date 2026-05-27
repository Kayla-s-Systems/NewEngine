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

    pub const F1: u32 = 159;
    pub const F2: u32 = 160;
}

/// Canonical key identity strings used by input profiles, binding catalogs and platform providers.
///
/// Platform plugins may report their native physical key name, but the authoritative mapping to
/// engine key codes lives here. This keeps platform backends from becoming hidden keybinding layers.
pub mod key_identity {
    use super::key_code;

    pub const DIGIT1: &str = "keyboard.digit1";
    pub const DIGIT2: &str = "keyboard.digit2";
    pub const DIGIT3: &str = "keyboard.digit3";

    pub const KEY_A: &str = "keyboard.key_a";
    pub const KEY_D: &str = "keyboard.key_d";
    pub const KEY_E: &str = "keyboard.key_e";
    pub const KEY_F: &str = "keyboard.key_f";
    pub const KEY_Q: &str = "keyboard.key_q";
    pub const KEY_S: &str = "keyboard.key_s";
    pub const KEY_W: &str = "keyboard.key_w";

    pub const ENTER: &str = "keyboard.enter";
    pub const SPACE: &str = "keyboard.space";
    pub const SHIFT_LEFT: &str = "keyboard.shift_left";
    pub const SHIFT_RIGHT: &str = "keyboard.shift_right";
    pub const TAB: &str = "keyboard.tab";
    pub const BACKSPACE: &str = "keyboard.backspace";

    pub const ARROW_LEFT: &str = "keyboard.arrow_left";
    pub const ARROW_UP: &str = "keyboard.arrow_up";
    pub const ARROW_RIGHT: &str = "keyboard.arrow_right";
    pub const ARROW_DOWN: &str = "keyboard.arrow_down";

    pub const ESCAPE: &str = "keyboard.escape";
    pub const F1: &str = "keyboard.f1";
    pub const F2: &str = "keyboard.f2";

    #[inline]
    pub fn key_code_from_id(id: &str) -> Option<u32> {
        match id.trim() {
            DIGIT1 => Some(key_code::DIGIT1),
            DIGIT2 => Some(key_code::DIGIT2),
            DIGIT3 => Some(key_code::DIGIT3),
            KEY_A => Some(key_code::KEY_A),
            KEY_D => Some(key_code::KEY_D),
            KEY_E => Some(key_code::KEY_E),
            KEY_F => Some(key_code::KEY_F),
            KEY_Q => Some(key_code::KEY_Q),
            KEY_S => Some(key_code::KEY_S),
            KEY_W => Some(key_code::KEY_W),
            ENTER => Some(key_code::ENTER),
            SPACE => Some(key_code::SPACE),
            SHIFT_LEFT => Some(key_code::SHIFT_LEFT),
            SHIFT_RIGHT => Some(key_code::SHIFT_RIGHT),
            TAB => Some(key_code::TAB),
            BACKSPACE => Some(key_code::BACKSPACE),
            ARROW_LEFT => Some(key_code::ARROW_LEFT),
            ARROW_UP => Some(key_code::ARROW_UP),
            ARROW_RIGHT => Some(key_code::ARROW_RIGHT),
            ARROW_DOWN => Some(key_code::ARROW_DOWN),
            ESCAPE => Some(key_code::ESCAPE),
            F1 => Some(key_code::F1),
            F2 => Some(key_code::F2),
            _ => None,
        }
    }

    /// Converts native physical key names used by common platform providers into canonical engine ids.
    ///
    /// This function intentionally lives in `newengine-input-api`; platform plugins should not own
    /// semantic key ids and must not assign gameplay/editor actions.
    #[inline]
    pub fn canonical_id_from_native_physical_name(name: &str) -> Option<&'static str> {
        match name.trim() {
            "Digit1" => Some(DIGIT1),
            "Digit2" => Some(DIGIT2),
            "Digit3" => Some(DIGIT3),
            "KeyA" => Some(KEY_A),
            "KeyD" => Some(KEY_D),
            "KeyE" => Some(KEY_E),
            "KeyF" => Some(KEY_F),
            "KeyQ" => Some(KEY_Q),
            "KeyS" => Some(KEY_S),
            "KeyW" => Some(KEY_W),
            "Enter" => Some(ENTER),
            "Space" => Some(SPACE),
            "ShiftLeft" => Some(SHIFT_LEFT),
            "ShiftRight" => Some(SHIFT_RIGHT),
            "Tab" => Some(TAB),
            "Backspace" => Some(BACKSPACE),
            "ArrowLeft" => Some(ARROW_LEFT),
            "ArrowUp" => Some(ARROW_UP),
            "ArrowRight" => Some(ARROW_RIGHT),
            "ArrowDown" => Some(ARROW_DOWN),
            "Escape" => Some(ESCAPE),
            "F1" => Some(F1),
            "F2" => Some(F2),
            _ => None,
        }
    }

    #[inline]
    pub fn key_code_from_native_physical_name(name: &str) -> Option<u32> {
        canonical_id_from_native_physical_name(name).and_then(key_code_from_id)
    }
}

/// Engine-owned default keyboard choices for non-user gameplay tooling actions.
///
/// These are still normal input bindings and are installed by input profiles. They are not platform
/// shortcuts, and platform backends must not special-case them.
pub mod engine_default_keybind {
    use super::key_code;

    pub const PRIMARY_UI_TOGGLE: u32 = key_code::ESCAPE;
    pub const ASSET_CATALOG_UI_TOGGLE: u32 = key_code::F1;
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
