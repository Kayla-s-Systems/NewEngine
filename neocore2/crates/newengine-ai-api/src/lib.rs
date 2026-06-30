#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO contract for the `engine.ai` gateway.
//!
//! AI observes stable snapshots and emits intents. The provider boundary never
//! receives `&mut World`, raw ECS storage or native EntityId. World/entity/ECS
//! mutation is owned by runtime apply stages.

use newengine_animation_api::AnimationIntentDtoV1;
use newengine_entity_api::EntityHandle;
use newengine_navigation_api::{NavPathDtoV1, NavVec3};
use newengine_tags_api::TagId;
use newengine_tasks_api::{TaskId, TaskRequestDtoV1};
use serde::{Deserialize, Serialize};

pub const ENGINE_AI_SERVICE_ID: &str = "engine.ai";
pub const AI_SERVICE_ID: &str = "ai.api";
pub const AI_BACKEND_CAPABILITY_ID: &str = "ai.backend";
pub const AI_RUNTIME_CONTRACT: &str = "newengine.ai-api/v1";

pub mod ai_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const FRAME_JSON_V1: &str = "ai.frame_json_v1";
    pub const VALIDATE_INTENTS_JSON_V1: &str = "ai.validate_intents_json_v1";
    pub const DECISION_TRACE_JSON_V1: &str = "ai.decision_trace_json_v1";
}

pub const AI_SERVICE_METHODS: &[&str] = &[
    ai_method::INFO_JSON,
    ai_method::INVOKE_JSON,
    ai_method::SHUTDOWN_V1,
    ai_method::FRAME_JSON_V1,
    ai_method::VALIDATE_INTENTS_JSON_V1,
    ai_method::DECISION_TRACE_JSON_V1,
];

pub const AI_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "ai",
        ENGINE_AI_SERVICE_ID,
        AI_SERVICE_ID,
        AI_BACKEND_CAPABILITY_ID,
    );

pub const AI_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_AI_SERVICE_ID,
        "newengine.ai-api >= 0.1.x",
        AI_SERVICE_METHODS,
    );

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiPerceptionFactV1 {
    pub fact_id: String,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiAgentSnapshotV1 {
    pub entity: EntityHandle,
    pub agent_id: String,
    #[serde(default)]
    pub position: Option<NavVec3>,
    #[serde(default)]
    pub velocity: Option<NavVec3>,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub current_task: Option<TaskId>,
    #[serde(default)]
    pub visible_facts: Vec<AiPerceptionFactV1>,
    #[serde(default)]
    pub blackboard: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AiIntentKind {
    MoveTo,
    FollowPath,
    PlayAnimation,
    RequestTask,
    AddTag,
    RemoveTag,
    EmitEvent,
    #[default]
    Idle,
    Custom(String),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiIntentDtoV1 {
    pub intent_id: String,
    pub agent: EntityHandle,
    #[serde(default)]
    pub kind: AiIntentKind,
    #[serde(default)]
    pub target_position: Option<NavVec3>,
    #[serde(default)]
    pub path: Option<NavPathDtoV1>,
    #[serde(default)]
    pub task: Option<TaskRequestDtoV1>,
    #[serde(default)]
    pub animation: Option<AnimationIntentDtoV1>,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiFrameInputV1 {
    pub frame_id: u64,
    #[serde(default)]
    pub fixed_tick: u64,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub agents: Vec<AiAgentSnapshotV1>,
    #[serde(default)]
    pub world_facts: Vec<AiPerceptionFactV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiDecisionTraceV1 {
    pub agent: EntityHandle,
    #[serde(default)]
    pub selected_pattern: String,
    #[serde(default)]
    pub score: f32,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AiFrameOutputV1 {
    pub accepted: bool,
    #[serde(default)]
    pub fixed_tick: u64,
    #[serde(default)]
    pub intents: Vec<AiIntentDtoV1>,
    #[serde(default)]
    pub decision_trace: Vec<AiDecisionTraceV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AiValidateIntentsRequestV1 {
    #[serde(default)]
    pub intents: Vec<AiIntentDtoV1>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AiValidateIntentsResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub intents: Vec<AiIntentDtoV1>,
    #[serde(default)]
    pub rejected: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiServiceInfoV1 {
    pub protocol: String,
    pub provider: String,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl Default for AiServiceInfoV1 {
    fn default() -> Self {
        Self {
            protocol: AI_RUNTIME_CONTRACT.to_owned(),
            provider: "engine.ai.foundation".to_owned(),
            methods: AI_SERVICE_METHODS
                .iter()
                .map(|it| (*it).to_owned())
                .collect(),
            features: vec![
                "frame-dto-input".to_owned(),
                "intent-dto-output".to_owned(),
                "no-direct-world-mutation".to_owned(),
                "deterministic-decision-trace".to_owned(),
            ],
        }
    }
}
