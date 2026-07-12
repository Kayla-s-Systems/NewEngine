use newengine_tags_api::TagId;
use serde::{Deserialize, Serialize};

use crate::{TaskDescriptorV1, TaskQueueSnapshotV1, TaskRequestDtoV1};

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
