#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO contract for the `engine.tasks` gateway.

mod model;
mod operations;
mod service;

pub use model::{TaskDescriptorV1, TaskId, TaskKind, TaskQueueSnapshotV1, TaskRequestDtoV1};
pub use operations::{
    TasksDescribeRequestV1, TasksDescribeResponseV1, TasksPlanQueueRequestV1,
    TasksPlanQueueResponseV1, TasksValidateRequestV1, TasksValidateResponseV1,
};
pub use service::{
    tasks_method, TasksServiceInfoV1, ENGINE_TASKS_SERVICE_ID, TASKS_BACKEND_CAPABILITY_ID,
    TASKS_BACKEND_SERVICE_SPEC, TASKS_RUNTIME_CONTRACT, TASKS_SERVICE_ID, TASKS_SERVICE_METHODS,
};
