use serde::{Deserialize, Serialize};

pub const ENGINE_TAGS_SERVICE_ID: &str = "engine.tags";
pub const TAGS_SERVICE_ID: &str = "tags.api";
pub const TAGS_REGISTRY_CAPABILITY_ID: &str = "tags.registry";
pub const TAGS_RUNTIME_CONTRACT: &str = "newengine.tags-api/v1";

pub mod tags_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const DESCRIBE_TAGS_JSON_V1: &str = "tags.describe_tags_json_v1";
    pub const RESOLVE_TAG_JSON_V1: &str = "tags.resolve_tag_json_v1";
    pub const SNAPSHOT_JSON_V1: &str = "tags.snapshot_json_v1";
    pub const VALIDATE_TAG_SET_JSON_V1: &str = "tags.validate_tag_set_json_v1";
}

pub const TAGS_SERVICE_METHODS: &[&str] = &[
    tags_method::INFO_JSON,
    tags_method::INVOKE_JSON,
    tags_method::SHUTDOWN_V1,
    tags_method::DESCRIBE_TAGS_JSON_V1,
    tags_method::RESOLVE_TAG_JSON_V1,
    tags_method::SNAPSHOT_JSON_V1,
    tags_method::VALIDATE_TAG_SET_JSON_V1,
];

pub const TAGS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "tags",
        ENGINE_TAGS_SERVICE_ID,
        TAGS_SERVICE_ID,
        TAGS_REGISTRY_CAPABILITY_ID,
    );

pub const TAGS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_TAGS_SERVICE_ID,
        "newengine.tags-api >= 0.1.x",
        TAGS_SERVICE_METHODS,
    );

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagsServiceInfoV1 {
    pub protocol: String,
    pub provider: String,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl Default for TagsServiceInfoV1 {
    fn default() -> Self {
        Self {
            protocol: TAGS_RUNTIME_CONTRACT.to_owned(),
            provider: "engine.tags.foundation".to_owned(),
            methods: TAGS_SERVICE_METHODS
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
            features: vec![
                "gameplay-vocabulary".to_owned(),
                "data-driven-tags".to_owned(),
            ],
        }
    }
}
