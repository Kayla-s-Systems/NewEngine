use crate::events::EventHub;
use newengine_loading_api::{EngineTaskEvent, ENGINE_TASK_EVENT_TOPIC_V1};

pub(super) fn publish_task_event(events: Option<&EventHub>, event: EngineTaskEvent) {
    if let Some(events) = events {
        let _ = events.publish(event.clone());
    }
    if let Ok(bytes) = serde_json::to_vec(&event) {
        let _ = newengine_plugin_host::emit_plugin_event(ENGINE_TASK_EVENT_TOPIC_V1, &bytes);
    }
}
