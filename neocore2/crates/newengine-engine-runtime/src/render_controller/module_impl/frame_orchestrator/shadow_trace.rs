use super::*;

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
            "render shadow plan: kind={} active={} render_this_frame={} cache_valid={} target={:?} resolution={}",
            shadow_kind,
            shadow_plan.is_active(),
            render_shadow_map,
            controller.shadows.cache_valid,
            shadow_plan.render_target(),
            shadow_plan.resolution
        );
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
