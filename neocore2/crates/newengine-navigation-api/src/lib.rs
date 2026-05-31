#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO contract for the `engine.navigation` gateway.

use newengine_entity_api::EntityHandle;
use newengine_tags_api::TagId;
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

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct NavVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl NavVec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavPathPointV1 {
    pub position: NavVec3,
    #[serde(default)]
    pub flags: Vec<TagId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavPlanPathRequestV1 {
    #[serde(default)]
    pub agent: Option<EntityHandle>,
    pub start: NavVec3,
    pub goal: NavVec3,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub constraints: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NavPathDtoV1 {
    #[serde(default)]
    pub points: Vec<NavPathPointV1>,
    #[serde(default)]
    pub cost: f32,
    #[serde(default)]
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NavPlanPathResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub path: Option<NavPathDtoV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavProjectPointRequestV1 {
    pub point: NavVec3,
    #[serde(default)]
    pub radius: f32,
    #[serde(default)]
    pub tags: Vec<TagId>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NavProjectPointResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub projected: Option<NavVec3>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NavQueryStatusRequestV1 {
    #[serde(default)]
    pub query_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NavQueryStatusResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

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
            methods: NAVIGATION_SERVICE_METHODS.iter().map(|it| (*it).to_owned()).collect(),
            features: vec!["path-query-dto".to_owned(), "no-world-mutation".to_owned()],
        }
    }
}
