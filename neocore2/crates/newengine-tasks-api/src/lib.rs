#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO contract for the `engine.tasks` gateway.

use newengine_entity_api::EntityHandle;
use newengine_tags_api::TagId;
use serde::{Deserialize, Serialize};

pub const ENGINE_TASKS_SERVICE_ID: &str = "engine.tasks";
pub const TASKS_SERVICE_ID: &str = "tasks.api";
pub const TASKS_BACKEND_CAPABILITY_ID: &str = "tasks.backend";
pub const TASKS_RUNTIME_CONTRACT: &str = "newengine.tasks-api/v1";

pub mod tasks_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const DESCRIBE_TASKS_JSON_V1: &str = "tasks.describe_tasks_json_v1";
    pub const VALIDATE_TASK_JSON_V1: &str = "tasks.validate_task_json_v1";
    pub const PLAN_QUEUE_JSON_V1: &str = "tasks.plan_queue_json_v1";
}

pub const TASKS_SERVICE_METHODS: &[&str] = &[
    tasks_method::INFO_JSON,
    tasks_method::INVOKE_JSON,
    tasks_method::SHUTDOWN_V1,
    tasks_method::DESCRIBE_TASKS_JSON_V1,
    tasks_method::VALIDATE_TASK_JSON_V1,
    tasks_method::PLAN_QUEUE_JSON_V1,
];

pub const TASKS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "tasks",
        ENGINE_TASKS_SERVICE_ID,
        TASKS_SERVICE_ID,
        TASKS_BACKEND_CAPABILITY_ID,
    );

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    #[inline]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    MoveTo,
    Wait,
    PlayAnimation,
    AttachEntity,
    RequestDialogue,
    ClaimResource,
    Custom(String),
}

impl Default for TaskKind {
    #[inline]
    fn default() -> Self {
        Self::Custom("unknown".to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskDescriptorV1 {
    pub task: TaskId,
    #[serde(default)]
    pub kind: TaskKind,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub required_parameters: Vec<String>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRequestDtoV1 {
    pub task: TaskId,
    #[serde(default)]
    pub issuer: Option<EntityHandle>,
    #[serde(default)]
    pub target: Option<EntityHandle>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<TagId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskQueueSnapshotV1 {
    pub owner: String,
    #[serde(default)]
    pub entity: Option<EntityHandle>,
    #[serde(default)]
    pub pending: Vec<TaskRequestDtoV1>,
    #[serde(default)]
    pub current: Option<TaskRequestDtoV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TasksDescribeRequestV1 {
    #[serde(default)]
    pub tag_filter: Vec<TagId>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TasksDescribeResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub tasks: Vec<TaskDescriptorV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TasksValidateRequestV1 {
    pub request: TaskRequestDtoV1,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TasksValidateResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub normalized: Option<TaskRequestDtoV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TasksPlanQueueRequestV1 {
    #[serde(default)]
    pub queues: Vec<TaskQueueSnapshotV1>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TasksPlanQueueResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub planned_queues: Vec<TaskQueueSnapshotV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TasksServiceInfoV1 {
    pub protocol: String,
    pub provider: String,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl Default for TasksServiceInfoV1 {
    fn default() -> Self {
        Self {
            protocol: TASKS_RUNTIME_CONTRACT.to_owned(),
            provider: "engine.tasks.foundation".to_owned(),
            methods: TASKS_SERVICE_METHODS
                .iter()
                .map(|it| (*it).to_owned())
                .collect(),
            features: vec![
                "declarative-task-language".to_owned(),
                "intent-friendly".to_owned(),
            ],
        }
    }
}
