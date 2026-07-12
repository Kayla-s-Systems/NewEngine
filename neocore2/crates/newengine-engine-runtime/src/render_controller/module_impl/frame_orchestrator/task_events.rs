use super::*;

static RENDER_JOB_EVENT_MODE: OnceLock<String> = OnceLock::new();
static RENDER_JOB_EVENT_INTERVAL: OnceLock<u64> = OnceLock::new();

impl RenderFrameOrchestrator {
    pub(in super::super) fn render_prep_executor_detail(
        thread_pool: Option<&ThreadPoolHandle>,
        detail: &'static str,
    ) -> String {
        match thread_pool {
            Some(jobs) => format!(
                "{detail} engine.threading available worker_threads={} pending_render_prep={}; target split: jobs build provider-safe packets, render thread submits GPU/backend envelope.",
                jobs.worker_threads(),
                jobs.pending_for_lane(newengine_core::TaskLane::RenderPrep),
            ),
            None => format!(
                "{detail} engine.threading handle unavailable for this frame; render-prep remains a main-thread barrier."
            ),
        }
    }

    pub(in super::super) fn should_publish_render_task_pass_event(frame_index: u64) -> bool {
        let mode = RENDER_JOB_EVENT_MODE.get_or_init(|| {
            crate::env_config::var("NEWENGINE_RENDER_JOB_EVENT_MODE")
                .unwrap_or_else(|| "sampled".to_owned())
                .trim()
                .to_ascii_lowercase()
        });
        match mode.as_str() {
            "off" | "none" | "disabled" => false,
            "full" | "all" | "trace" => true,
            _ => {
                let interval = *RENDER_JOB_EVENT_INTERVAL.get_or_init(|| {
                    crate::env_config::var_u64(
                        "NEWENGINE_RENDER_JOB_EVENT_INTERVAL_FRAMES",
                        120,
                        1,
                        6000,
                    )
                });
                frame_index <= 3 || frame_index.is_multiple_of(interval)
            }
        }
    }

    pub(in super::super) fn publish_render_task_pass_event(
        frame_index: u64,
        pass: &'static str,
        phase: newengine_task_api::EngineTaskPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) {
        if !Self::should_publish_render_task_pass_event(frame_index) {
            return;
        }

        let mut event = newengine_task_api::EngineTaskEvent::new(
            format!("render.frame.{frame_index}.{pass}"),
            "render.frame-orchestrator",
            "engine.render",
            "render",
            format!("render:{pass}"),
            "render-prep",
            phase,
            status.into(),
            detail.into(),
        )
        .with_frame_id(frame_index)
        .with_dependency_group(format!("frame.{frame_index}.render"))
        .with_task_domain(newengine_task_api::task_domain::ENGINE_RENDER)
        .with_task_pass(pass)
        .with_priority("critical")
        .with_executor("main-thread-barrier")
        .with_controls(false, false);
        if let Some(progress) = progress_01 {
            event = event.with_progress(progress);
        }
        let event_payload = serde_json::to_vec(&event).ok();
        let job_event = newengine_task_api::EngineTaskEnvelopeV1::new(
            event,
            newengine_task_api::TaskExecutorKind::MainThreadBarrier,
            "render-frame-job-pass",
        );
        if let Some(payload) = event_payload {
            let _ = newengine_plugin_host::host_context::publish_event(
                newengine_task_api::ENGINE_TASK_EVENT_TOPIC_V1,
                &payload,
            );
        }
        if let Ok(payload) = serde_json::to_vec(&job_event) {
            let _ = newengine_plugin_host::host_context::publish_event(
                newengine_task_api::ENGINE_TASK_ENVELOPE_TOPIC_V1,
                &payload,
            );
        }
    }
}
