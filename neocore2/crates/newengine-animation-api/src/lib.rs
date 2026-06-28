#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO contract for the `engine.animation` gateway.

use newengine_entity_api::EntityHandle;
use newengine_tags_api::TagId;
use newengine_tasks_api::TaskId;
use serde::{Deserialize, Serialize};

pub const ENGINE_ANIMATION_SERVICE_ID: &str = "engine.animation";
pub const ANIMATION_SERVICE_ID: &str = "animation.api";
pub const ANIMATION_BACKEND_CAPABILITY_ID: &str = "animation.backend";
pub const ANIMATION_RUNTIME_CONTRACT: &str = "newengine.animation-api/v1";

pub mod animation_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const DESCRIBE_GRAPHS_JSON_V1: &str = "animation.describe_graphs_json_v1";
    pub const PLAN_JSON_V1: &str = "animation.plan_json_v1";
    pub const VALIDATE_INTENT_JSON_V1: &str = "animation.validate_intent_json_v1";
}

pub const ANIMATION_SERVICE_METHODS: &[&str] = &[
    animation_method::INFO_JSON,
    animation_method::INVOKE_JSON,
    animation_method::SHUTDOWN_V1,
    animation_method::DESCRIBE_GRAPHS_JSON_V1,
    animation_method::PLAN_JSON_V1,
    animation_method::VALIDATE_INTENT_JSON_V1,
];

pub const ANIMATION_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "animation",
        ENGINE_ANIMATION_SERVICE_ID,
        ANIMATION_SERVICE_ID,
        ANIMATION_BACKEND_CAPABILITY_ID,
    );

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AnimationGraphRef(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AnimationClipRef(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnimationIntentKind {
    PlayClip,
    Stop,
    BlendToState,
    SetParameter,
    AttachTask,
    Custom(String),
}

impl Default for AnimationIntentKind {
    fn default() -> Self {
        Self::PlayClip
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationIntentDtoV1 {
    pub entity: EntityHandle,
    #[serde(default)]
    pub intent: AnimationIntentKind,
    #[serde(default)]
    pub graph: Option<AnimationGraphRef>,
    #[serde(default)]
    pub clip: Option<AnimationClipRef>,
    #[serde(default)]
    pub task: Option<TaskId>,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationGraphDescriptorV1 {
    pub graph: AnimationGraphRef,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub states: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationDescribeGraphsRequestV1 {
    #[serde(default)]
    pub tag_filter: Vec<TagId>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationDescribeGraphsResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub graphs: Vec<AnimationGraphDescriptorV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationPlanRequestV1 {
    #[serde(default)]
    pub intents: Vec<AnimationIntentDtoV1>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationPlanResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub accepted_intents: Vec<AnimationIntentDtoV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationValidateIntentRequestV1 {
    pub intent: AnimationIntentDtoV1,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationValidateIntentResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub normalized: Option<AnimationIntentDtoV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationServiceInfoV1 {
    pub protocol: String,
    pub provider: String,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl Default for AnimationServiceInfoV1 {
    fn default() -> Self {
        Self {
            protocol: ANIMATION_RUNTIME_CONTRACT.to_owned(),
            provider: "engine.animation.foundation".to_owned(),
            methods: ANIMATION_SERVICE_METHODS
                .iter()
                .map(|it| (*it).to_owned())
                .collect(),
            features: vec!["animation-intents".to_owned(), "task-bindings".to_owned()],
        }
    }
}
