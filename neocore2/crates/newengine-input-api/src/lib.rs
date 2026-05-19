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
