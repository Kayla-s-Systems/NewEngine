#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use newengine_math::collections_prelude::NeHashMap as HashMap;
use newengine_task_api::{
    EngineTaskEnvelopeV1, EngineTaskEvent, EngineTaskPhase, TaskExecutorKind,
    ENGINE_TASK_ENVELOPE_TOPIC_V1, ENGINE_TASK_EVENT_TOPIC_V1,
};

static HOST_JOB_SEQ: AtomicU64 = AtomicU64::new(1);
static ACTIVE_HOST_JOBS: OnceLock<Mutex<HashMap<String, PluginHostJobRecord>>> = OnceLock::new();

#[derive(Clone, Debug)]
struct PluginHostJobRecord {
    task_id: String,
    name: String,
    category: String,
    source: String,
    owner: String,
    lane: String,
    semantic_owner: String,
    can_pause: bool,
    can_cancel: bool,
}

impl PluginHostJobRecord {
    fn from_json(value: &serde_json::Value) -> Self {
        let task_id = str_field(value, "id", "host.task.unknown").to_owned();
        let category = str_field(value, "category", "plugin-host").to_owned();
        let owner = owner_from_json(value).to_owned();
        let name = str_field(value, "name", task_id.as_str()).to_owned();
        Self {
            task_id,
            name,
            category: category.clone(),
            source: str_field(value, "source", "newengine-plugin-host").to_owned(),
            owner,
            lane: lane_for_category(category.as_str()).to_owned(),
            semantic_owner: semantic_owner_for_category(category.as_str()).to_owned(),
            can_pause: bool_field(value, "can_pause", false),
            can_cancel: bool_field(value, "can_cancel", false),
        }
    }

    fn task_event(
        &self,
        phase: EngineTaskPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) -> EngineTaskEvent {
        let mut event = EngineTaskEvent::new(
            self.task_id.clone(),
            self.source.clone(),
            self.owner.clone(),
            self.category.clone(),
            self.name.clone(),
            self.lane.clone(),
            phase,
            status,
            detail,
        )
        .with_controls(self.can_pause, self.can_cancel);
        if let Some(progress) = progress_01 {
            event = event.with_progress(progress);
        }
        event
    }
}

pub(crate) struct PluginHostJobBridge;

impl PluginHostJobBridge {
    #[inline]
    fn active_jobs() -> &'static Mutex<HashMap<String, PluginHostJobRecord>> {
        ACTIVE_HOST_JOBS.get_or_init(|| Mutex::new(HashMap::default()))
    }

    fn begin(value: serde_json::Value) {
        let record = PluginHostJobRecord::from_json(&value);
        if let Ok(mut active) = Self::active_jobs().lock() {
            active.insert(record.task_id.clone(), record.clone());
        }
        let detail = str_field(
            &value,
            "detail",
            "Plugin-host work entered the engine.jobs bridge.",
        );
        Self::publish(
            &record,
            EngineTaskPhase::Scheduled,
            "Job scheduled",
            detail,
            Some(0.0),
        );
        Self::publish(
            &record,
            EngineTaskPhase::Running,
            "Job running",
            detail,
            None,
        );
    }

    fn end(value: serde_json::Value) {
        let task_id = str_field(&value, "id", "host.task.unknown").to_owned();
        let record = Self::active_jobs()
            .lock()
            .ok()
            .and_then(|mut active| active.remove(task_id.as_str()))
            .unwrap_or_else(|| PluginHostJobRecord::from_json(&value));

        let phase = match value
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
        {
            "completed" => EngineTaskPhase::Completed,
            "cancelled" | "canceled" => EngineTaskPhase::Cancelled,
            _ => EngineTaskPhase::Failed,
        };
        let status = match phase {
            EngineTaskPhase::Completed => "Job completed",
            EngineTaskPhase::Cancelled => "Job cancelled",
            EngineTaskPhase::Failed => "Job failed",
            _ => phase.state_label(),
        };
        let detail = str_field(&value, "detail", phase.state_label());
        Self::publish(&record, phase, status, detail, Some(1.0));
    }

    fn publish(
        record: &PluginHostJobRecord,
        phase: EngineTaskPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) {
        let event = record.task_event(phase, status, detail, progress_01);
        let job_event = EngineTaskEnvelopeV1::new(
            event.clone(),
            TaskExecutorKind::PluginHostBridge,
            record.semantic_owner.clone(),
        );
        if let Ok(bytes) = serde_json::to_vec(&event) {
            let _ = crate::host_context::publish_event(ENGINE_TASK_EVENT_TOPIC_V1, &bytes);
        }
        if let Ok(bytes) = serde_json::to_vec(&job_event) {
            let _ = crate::host_context::publish_event(ENGINE_TASK_ENVELOPE_TOPIC_V1, &bytes);
        }
    }
}

#[inline]
pub(crate) fn next_job_id(prefix: &str) -> String {
    format!(
        "{}.{}",
        prefix,
        HOST_JOB_SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

#[inline]
pub(crate) fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

#[inline]
pub(crate) fn begin(value: serde_json::Value) {
    PluginHostJobBridge::begin(value);
}

#[inline]
pub(crate) fn end(value: serde_json::Value) {
    PluginHostJobBridge::end(value);
}

fn owner_from_json(value: &serde_json::Value) -> &str {
    value
        .get("owner_plugin_id")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("plugin_id").and_then(|v| v.as_str()))
        .or_else(|| value.get("service_id").and_then(|v| v.as_str()))
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|m| m.get("owner_plugin_id"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|m| m.get("plugin_id"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|m| m.get("service_id"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("newengine-plugin-host")
}

fn lane_for_category(category: &str) -> &'static str {
    match category {
        "service_call" | "plugin_lifecycle" => "plugin",
        "asset_io" | "asset" => "asset-io",
        "simulation" => "simulation",
        _ => "background",
    }
}

fn semantic_owner_for_category(category: &str) -> &'static str {
    match category {
        "service_call" => "plugin-host-service-call",
        "plugin_lifecycle" => "plugin-host-lifecycle",
        "asset_io" | "asset" => "asset-job",
        "simulation" => "simulation-job",
        _ => "plugin-host-work",
    }
}

fn str_field<'a>(value: &'a serde_json::Value, key: &str, fallback: &'a str) -> &'a str {
    value.get(key).and_then(|v| v.as_str()).unwrap_or(fallback)
}

fn bool_field(value: &serde_json::Value, key: &str, fallback: bool) -> bool {
    value.get(key).and_then(|v| v.as_bool()).unwrap_or(fallback)
}
