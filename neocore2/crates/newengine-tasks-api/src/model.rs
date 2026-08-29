use newengine_entity_api::EntityHandle;
use newengine_tags_api::TagId;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_round_trips_borrowed_text() {
        let task = TaskId::new("move.to");
        assert_eq!(task.as_str(), "move.to");
    }
}
