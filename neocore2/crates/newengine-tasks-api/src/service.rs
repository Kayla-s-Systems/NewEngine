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

const TASK_FEATURES: &[&str] = &["declarative-task-language", "intent-friendly"];

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
                .map(|method| (*method).to_owned())
                .collect(),
            features: TASK_FEATURES
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks_contract_is_gateway_first() {
        assert_eq!(ENGINE_TASKS_SERVICE_ID, "engine.tasks");
        assert_eq!(
            TASKS_BACKEND_SERVICE_SPEC.engine_gateway_id,
            ENGINE_TASKS_SERVICE_ID
        );
        assert_eq!(
            TasksServiceInfoV1::default().methods.len(),
            TASKS_SERVICE_METHODS.len()
        );
    }
}
