use newengine_entity_api::EntityHandle;
use newengine_tags_api::TagId;
use serde::{Deserialize, Serialize};

use crate::{NavPathDtoV1, NavVec3};

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
