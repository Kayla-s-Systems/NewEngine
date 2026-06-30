use newengine_core::TaskRuntimeStatus;
use newengine_task_api::TaskStatusJsonV1;

pub(crate) fn status_from_core(status: TaskRuntimeStatus) -> TaskStatusJsonV1 {
    TaskStatusJsonV1 {
        task_id: status.task_id,
        name: status.label.to_owned(),
        lane: status.lane.as_str().to_owned(),
        priority: status.priority.as_str().to_owned(),
        frame_id: status.frame_id,
        dependency_group: status.dependency_group.unwrap_or_default(),
        task_pass: status.task_pass.to_owned(),
        task_domain: status.task_domain.to_owned(),
        phase: status.phase,
        can_pause: status.can_pause,
        can_cancel: status.can_cancel,
        cancel_requested: status.cancel_requested,
        pause_requested: status.pause_requested,
        found: true,
    }
}

pub(crate) fn missing_status(task_id: impl Into<String>) -> TaskStatusJsonV1 {
    TaskStatusJsonV1 {
        task_id: task_id.into(),
        found: false,
        ..Default::default()
    }
}
