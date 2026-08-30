use newengine_core::{engine::EngineFrameTimingTelemetry, render::RenderModuleTimingTelemetry};
use newengine_ui_api::UiLayerDomain;

const PROFILER_SAMPLE_TOPIC: &str = "newengine.diagnostics.profiler.sample.v1";

pub(super) struct HostFrameProfileInput {
    pub(super) ui_frame_index: u64,
    pub(super) host_total_ms: f64,
    pub(super) input_dispatch_ms: f64,
    pub(super) input_poll_ms: f64,
    pub(super) ui_provider_dispatch_ms: f64,
    pub(super) ui_provider_dispatch_used: bool,
    pub(super) ui_prepare_ms: f64,
    pub(super) engine_step_ms: f64,
    pub(super) provider_ui_refresh: bool,
    pub(super) active_ui_domain: UiLayerDomain,
    pub(super) active_ui_surface_count: usize,
    pub(super) active_ui_invalidation_revision: u64,
    pub(super) gameplay_hud_refresh_due: bool,
    pub(super) ui_dispatch_refresh: bool,
    pub(super) screen_profile_refresh: bool,
    pub(super) screen_profile_game_refresh: bool,
    pub(super) screen_profile_shell_refresh: bool,
}

pub(super) fn publish_running_frame_samples(
    engine_timing: Option<&EngineFrameTimingTelemetry>,
    render_timing: Option<&RenderModuleTimingTelemetry>,
    host: HostFrameProfileInput,
) {
    let pacing_wait_ms = render_timing
        .filter(|render| {
            engine_timing.is_some_and(|engine| render.frame_index.abs_diff(engine.frame_index) <= 1)
        })
        .map(|render| {
            f64::from(render.backend_frame_slot_wait_ms)
                + f64::from(render.backend_surface_acquire_ms)
                + f64::from(render.backend_image_wait_ms)
        })
        .unwrap_or(0.0);

    if let Some(timing) = engine_timing {
        let active_cpu_ms = (timing.total_ms - pacing_wait_ms).max(0.0);
        let missed_frame = timing.total_ms >= 20.0;
        if active_cpu_ms >= 12.0 || missed_frame || timing.frame_index.is_multiple_of(120) {
            let mut payload = serde_json::json!({
                "schema": "newengine.diagnostics.profiler.sample.v1",
                "category": "engine.frame",
                "source": "newengine-core",
                "name": "engine frame orchestration",
                "lane": "main-frame",
                "priority": "critical",
                "dependency_group": format!("engine.frame.{}", timing.frame_index),
                "frame_index": timing.frame_index,
                "elapsed_ms": active_cpu_ms,
                "wall_elapsed_ms": timing.total_ms,
                "pacing_wait_ms": pacing_wait_ms,
                "budget_ms": 16.67,
                "frame_budget_ms": 16.67,
                "exceeded_frame_budget": active_cpu_ms > 16.67,
                "missed_wall_frame": missed_frame,
                "fixed_steps": timing.fixed_steps,
                "time_begin_ms": timing.time_begin_ms,
                "plugin_control_ms": timing.plugin_control_ms,
                "fixed_time_ms": timing.fixed_time_ms,
                "fixed_scheduler_ms": timing.fixed_scheduler_ms,
                "fixed_plugins_ms": timing.fixed_plugins_ms,
                "fixed_modules_ms": timing.fixed_modules_ms,
                "update_scheduler_ms": timing.update_scheduler_ms,
                "update_plugins_ms": timing.update_plugins_ms,
                "update_modules_ms": timing.update_modules_ms,
                "render_scheduler_ms": timing.render_scheduler_ms,
                "render_plugins_ms": timing.render_plugins_ms,
                "render_modules_ms": timing.render_modules_ms,
                "scheduler_end_ms": timing.scheduler_end_ms,
                "render_timing_frame_index": render_timing.map(|it| it.frame_index),
                "render_pre_begin_ms": render_timing.map(|it| it.pre_begin_ms),
                "render_backend_begin_ms": render_timing.map(|it| it.backend_begin_ms),
                "render_playable_frame_ms": render_timing.map(|it| it.playable_frame_ms),
                "render_diagnostics_before_present_ms": render_timing.map(|it| it.diagnostics_before_present_ms),
                "render_backend_end_ms": render_timing.map(|it| it.backend_end_ms),
                "backend_reported_begin_ms": render_timing.map(|it| it.backend_reported_begin_ms),
                "backend_frame_slot_wait_ms": render_timing.map(|it| it.backend_frame_slot_wait_ms),
                "backend_surface_acquire_ms": render_timing.map(|it| it.backend_surface_acquire_ms),
                "backend_image_wait_ms": render_timing.map(|it| it.backend_image_wait_ms),
                "backend_reported_end_ms": render_timing.map(|it| it.backend_reported_end_ms),
            });
            append_render_gpu_fields(&mut payload, render_timing);
            publish_sample(&payload);
        }
    }

    let host_active_cpu_ms = (host.host_total_ms - pacing_wait_ms).max(0.0);
    let host_wall_slow = host.host_total_ms >= 20.0;
    if host_active_cpu_ms >= 12.0 || host_wall_slow || host.ui_frame_index.is_multiple_of(120) {
        let payload = serde_json::json!({
            "schema": "newengine.diagnostics.profiler.sample.v1",
            "category": "host.frame",
            "source": "newengine-runtime-host",
            "name": "running host frame",
            "lane": "main-frame",
            "priority": "critical",
            "dependency_group": format!("host.frame.{}", host.ui_frame_index),
            "frame_index": host.ui_frame_index,
            "elapsed_ms": host_active_cpu_ms,
            "wall_elapsed_ms": host.host_total_ms,
            "pacing_wait_ms": pacing_wait_ms,
            "budget_ms": 16.67,
            "frame_budget_ms": 16.67,
            "exceeded_frame_budget": host_active_cpu_ms > 16.67,
            "missed_wall_frame": host_wall_slow,
            "input_dispatch_ms": host.input_dispatch_ms,
            "input_poll_ms": host.input_poll_ms,
            "ui_provider_dispatch_ms": host.ui_provider_dispatch_ms,
            "ui_provider_dispatch_used": host.ui_provider_dispatch_used,
            "ui_prepare_ms": host.ui_prepare_ms,
            "engine_step_ms": host.engine_step_ms,
            "render_timing_frame_index": render_timing.map(|it| it.frame_index),
            "provider_ui_refresh": host.provider_ui_refresh,
            "ui_layer_domain": host.active_ui_domain.as_str(),
            "ui_layer_surface_count": host.active_ui_surface_count,
            "ui_layer_invalidation_revision": host.active_ui_invalidation_revision,
            "gameplay_hud_refresh_due": host.gameplay_hud_refresh_due,
            "ui_dispatch_refresh": host.ui_dispatch_refresh,
            "screen_profile_refresh": host.screen_profile_refresh,
            "screen_profile_game_refresh": host.screen_profile_game_refresh,
            "screen_profile_shell_refresh": host.screen_profile_shell_refresh,
        });
        publish_sample(&payload);
    }
}

fn append_render_gpu_fields(
    payload: &mut serde_json::Value,
    render_timing: Option<&RenderModuleTimingTelemetry>,
) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    for (key, value) in [
        (
            "backend_gpu_timestamps_enabled",
            serde_json::json!(render_timing.map(|it| it.backend_gpu_timestamps_enabled)),
        ),
        (
            "backend_gpu_timing_frame_index",
            serde_json::json!(render_timing.map(|it| it.backend_gpu_timing_frame_index)),
        ),
        (
            "backend_gpu_shadow_ms",
            serde_json::json!(render_timing.map(|it| it.backend_gpu_shadow_ms)),
        ),
        (
            "backend_gpu_opaque_ms",
            serde_json::json!(render_timing.map(|it| it.backend_gpu_opaque_ms)),
        ),
        (
            "backend_gpu_postfx_ms",
            serde_json::json!(render_timing.map(|it| it.backend_gpu_postfx_ms)),
        ),
        (
            "backend_gpu_ui_ms",
            serde_json::json!(render_timing.map(|it| it.backend_gpu_ui_ms)),
        ),
        (
            "backend_gpu_profiled_ms",
            serde_json::json!(render_timing.map(|it| it.backend_gpu_profiled_ms)),
        ),
    ] {
        object.insert(key.to_owned(), value);
    }
}

#[inline]
fn publish_sample(payload: &serde_json::Value) {
    if let Ok(bytes) = serde_json::to_vec(payload) {
        let _ = newengine_plugin_host::host_context::publish_event(PROFILER_SAMPLE_TOPIC, &bytes);
    }
}
