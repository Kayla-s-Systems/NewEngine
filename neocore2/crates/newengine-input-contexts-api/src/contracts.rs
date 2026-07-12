use super::*;

pub const ENGINE_INPUT_CONTEXTS_SERVICE_ID: &str = "engine.input.contexts";
pub const INPUT_CONTEXTS_SERVICE_ID: &str = "input.contexts.api";
pub const INPUT_CONTEXTS_BACKEND_CAPABILITY_ID: &str = "input.contexts.backend";

pub const INPUT_CONTEXTS_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const INPUT_CONTEXTS_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const INPUT_CONTEXTS_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const INPUT_CONTEXTS_METHOD_STACK_JSON_V1: &str = "stack_json_v1";
pub const INPUT_CONTEXTS_METHOD_PUSH_JSON_V1: &str = "push_json_v1";
pub const INPUT_CONTEXTS_METHOD_POP_JSON_V1: &str = "pop_json_v1";

pub const INPUT_CONTEXTS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "input.contexts",
        ENGINE_INPUT_CONTEXTS_SERVICE_ID,
        INPUT_CONTEXTS_SERVICE_ID,
        INPUT_CONTEXTS_BACKEND_CAPABILITY_ID,
    );

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputContextsServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for InputContextsServiceInfo {
    fn default() -> Self {
        Self {
            protocol: "newengine.input-contexts/v1".to_owned(),
            features: vec![
                "context-stack".to_owned(),
                "modal-capture".to_owned(),
                "priority-consume-policy".to_owned(),
            ],
            methods: vec![
                INPUT_CONTEXTS_METHOD_INFO.to_owned(),
                INPUT_CONTEXTS_METHOD_INVOKE.to_owned(),
                INPUT_CONTEXTS_METHOD_SHUTDOWN_V1.to_owned(),
                INPUT_CONTEXTS_METHOD_STACK_JSON_V1.to_owned(),
                INPUT_CONTEXTS_METHOD_PUSH_JSON_V1.to_owned(),
                INPUT_CONTEXTS_METHOD_POP_JSON_V1.to_owned(),
            ],
        }
    }
}
