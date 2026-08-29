use super::*;

pub const ENGINE_INPUT_BINDINGS_SERVICE_ID: &str = "engine.input.bindings";
pub const INPUT_BINDINGS_SERVICE_ID: &str = "input.bindings.api";
pub const INPUT_BINDINGS_BACKEND_CAPABILITY_ID: &str = "input.bindings.backend";

pub const INPUT_BINDINGS_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const INPUT_BINDINGS_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const INPUT_BINDINGS_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const INPUT_BINDINGS_METHOD_PROFILE_JSON_V1: &str = "profile_json_v1";
pub const INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1: &str = "save_profile_json_v1";
pub const INPUT_BINDINGS_METHOD_RESET_PROFILE_JSON_V1: &str = "reset_profile_json_v1";
pub const INPUT_BINDINGS_METHOD_ACTION_CATALOG_JSON_V1: &str = "action_catalog_json_v1";
pub const INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1: &str = "key_catalog_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_KEY_JSON_V1: &str = "register_key_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_ACTION_JSON_V1: &str = "register_action_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_BINDING_JSON_V1: &str = "register_binding_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_LISTENER_JSON_V1: &str = "register_listener_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1: &str = "register_manifest_json_v1";

pub const INPUT_BINDINGS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "input.bindings",
        ENGINE_INPUT_BINDINGS_SERVICE_ID,
        INPUT_BINDINGS_SERVICE_ID,
        INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
    );

pub const INPUT_BINDINGS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_INPUT_BINDINGS_SERVICE_ID,
        "newengine.input-bindings-api >= 0.1.x",
        &[
            newengine_service_api::SERVICE_METHOD_INFO_JSON,
            newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
            newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
            INPUT_BINDINGS_METHOD_PROFILE_JSON_V1,
            INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_KEY_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_ACTION_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_BINDING_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_LISTENER_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1,
        ],
    );

pub const INPUT_BINDINGS_RUNTIME_REQUIREMENT_SPEC:
    newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        INPUT_BINDINGS_RUNTIME_CONTRACT_SPEC,
        Some(INPUT_BINDINGS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_INPUT_BINDINGS"),
    );

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputBindingsServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for InputBindingsServiceInfo {
    fn default() -> Self {
        Self {
            protocol: "newengine.input-bindings/v1".to_owned(),
            features: vec![
                "central-key-registry".to_owned(),
                "central-action-registry".to_owned(),
                "semantic-actions".to_owned(),
                "action-listeners".to_owned(),
                "listener-priority-consumption".to_owned(),
                "manifest-registration".to_owned(),
                "gamepad-bindings".to_owned(),
                "device-preference".to_owned(),
            ],
            methods: vec![
                INPUT_BINDINGS_METHOD_INFO.to_owned(),
                INPUT_BINDINGS_METHOD_INVOKE.to_owned(),
                INPUT_BINDINGS_METHOD_SHUTDOWN_V1.to_owned(),
                INPUT_BINDINGS_METHOD_PROFILE_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_RESET_PROFILE_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_ACTION_CATALOG_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_KEY_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_ACTION_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_BINDING_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_LISTENER_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1.to_owned(),
            ],
        }
    }
}
