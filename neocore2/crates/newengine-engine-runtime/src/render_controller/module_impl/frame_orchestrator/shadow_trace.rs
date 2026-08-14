use super::*;

const PROFILER_SAMPLE_TOPIC: &str = "newengine.diagnostics.profiler.sample.v1";

impl RenderFrameOrchestrator {
    pub(in super::super) fn trace_shadow_plan(
        controller: &RuntimeRenderController,
        trace_frame: bool,
        shadow_plan: shadows::LightShadowPlan,
        render_shadow_map: bool,
    ) {
        if !trace_frame {
            return;
        }
        let shadow_kind = shadow_plan
            .light_kind
            .map(|kind| kind.label())
            .unwrap_or("none");
        newengine_ulog_api::ulog::debug!(
            "render shadow plan: kind={} active={} render_this_frame={} cache_valid={} target={:?} resolution={} cache(period={} reuse={} cold={} projection={} safety={})",
            shadow_kind,
            shadow_plan.is_active(),
            render_shadow_map,
            controller.shadows.cache_valid,
            shadow_plan.render_target(),
            shadow_plan.resolution,
            controller.shadows.refresh_period_frames,
            controller.shadows.cache_reuse_count,
            controller.shadows.cache_cold_refresh_count,
            controller.shadows.cache_projection_refresh_count,
            controller.shadows.cache_safety_refresh_count,
        );

        if crate::env_config::var_bool("NEWENGINE_RENDER_PROFILER_SAMPLES", true) {
            let payload = serde_json::json!({
                "schema": "newengine.diagnostics.profiler.sample.v1",
                "category": "shadow-cache",
                "source": "render_controller",
                "name": "shadow cache state",
                "detail": shadow_kind,
                "lane": "render-prep",
                "priority": "diagnostic",
                "frame_index": controller.frame.frame_index,
                "elapsed_ms": 0.0,
                "slow": false,
                "render_shadow_map": render_shadow_map,
                "cache_valid": controller.shadows.cache_valid,
                "refresh_period_frames": controller.shadows.refresh_period_frames,
                "cache_reuse_count": controller.shadows.cache_reuse_count,
                "cache_cold_refresh_count": controller.shadows.cache_cold_refresh_count,
                "cache_projection_refresh_count": controller.shadows.cache_projection_refresh_count,
                "cache_safety_refresh_count": controller.shadows.cache_safety_refresh_count,
                "resolution": shadow_plan.resolution,
            });
            if let Ok(bytes) = serde_json::to_vec(&payload) {
                let _ = newengine_plugin_host::emit_plugin_event(PROFILER_SAMPLE_TOPIC, &bytes);
            }
        }
    }
}

static GPU_SAFE_PROFILE_LOGGED: AtomicBool = AtomicBool::new(false);

pub(in super::super) fn log_gpu_safe_profile_once() {
    if GPU_SAFE_PROFILE_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        newengine_ulog_api::ulog::warn!(
            "render controller: legacy conservative GPU profile active; high-cost feature branches are disabled only by explicit runtime profile policy"
        );
        newengine_core::crash::record_breadcrumb(
            "render controller: legacy conservative GPU profile active",
        );
    }
}
