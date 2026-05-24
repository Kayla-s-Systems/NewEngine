use crate::events::EventHub;
use newengine_jobs_api::{EngineJobEventV1, JobExecutorKind, ENGINE_JOB_EVENT_TOPIC_V1};
use newengine_loading_api::{EngineTaskEvent, ENGINE_TASK_EVENT_TOPIC_V1};

pub(super) fn publish_task_event(events: Option<&EventHub>, event: EngineTaskEvent) {
    let job_event = EngineJobEventV1::new(
        event.clone(),
        JobExecutorKind::EngineWorker,
        "engine-owned-cpu-work",
    );

    if let Some(events) = events {
        let _ = events.publish(event.clone());
        let _ = events.publish(job_event.clone());
    }
    if let Ok(bytes) = serde_json::to_vec(&event) {
        let _ = newengine_plugin_host::emit_plugin_event(ENGINE_TASK_EVENT_TOPIC_V1, &bytes);
    }
    if let Ok(bytes) = serde_json::to_vec(&job_event) {
        let _ = newengine_plugin_host::emit_plugin_event(ENGINE_JOB_EVENT_TOPIC_V1, &bytes);
    }
}
