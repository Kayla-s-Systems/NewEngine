use serde::{Deserialize, Serialize};

pub const ENGINE_NAVIGATION_SERVICE_ID: &str = "engine.navigation";
pub const NAVIGATION_SERVICE_ID: &str = "navigation.api";
pub const NAVIGATION_BACKEND_CAPABILITY_ID: &str = "navigation.backend";
pub const NAVIGATION_RUNTIME_CONTRACT: &str = "newengine.navigation-api/v1";

pub mod navigation_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const PLAN_PATH_JSON_V1: &str = "navigation.plan_path_json_v1";
    pub const PROJECT_POINT_JSON_V1: &str = "navigation.project_point_json_v1";
    pub const QUERY_STATUS_JSON_V1: &str = "navigation.query_status_json_v1";
}

pub const NAVIGATION_SERVICE_METHODS: &[&str] = &[
    navigation_method::INFO_JSON,
    navigation_method::INVOKE_JSON,
    navigation_method::SHUTDOWN_V1,
    navigation_method::PLAN_PATH_JSON_V1,
    navigation_method::PROJECT_POINT_JSON_V1,
    navigation_method::QUERY_STATUS_JSON_V1,
];

pub const NAVIGATION_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "navigation",
        ENGINE_NAVIGATION_SERVICE_ID,
        NAVIGATION_SERVICE_ID,
        NAVIGATION_BACKEND_CAPABILITY_ID,
    );

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationServiceInfoV1 {
    pub protocol: String,
    pub provider: String,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl Default for NavigationServiceInfoV1 {
    fn default() -> Self {
        Self {
            protocol: NAVIGATION_RUNTIME_CONTRACT.to_owned(),
            provider: "engine.navigation.foundation".to_owned(),
            methods: NAVIGATION_SERVICE_METHODS
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
            features: vec!["path-query-dto".to_owned(), "no-world-mutation".to_owned()],
        }
    }
}
