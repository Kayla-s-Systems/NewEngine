use super::*;
use crate::render_controller::module_impl::frame_envelope_builder::build_ui_layer_frame_envelope;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DegradedViewportEnd {
    PipelineWait,
    PipelineFailure,
    DrawFailure,
}

impl DegradedViewportEnd {
    #[inline]
    const fn discard_commands(self) -> bool {
        matches!(self, Self::PipelineWait | Self::DrawFailure)
    }

    #[inline]
    const fn restore_surface_viewport(self) -> bool {
        !matches!(self, Self::DrawFailure)
    }

    #[inline]
    const fn collect_garbage(self) -> bool {
        !matches!(self, Self::DrawFailure)
    }

    #[inline]
    const fn breadcrumb_suffix(self) -> &'static str {
        match self {
            Self::PipelineWait => "while material shader pipeline is pending",
            Self::PipelineFailure => "after viewport disable",
            Self::DrawFailure => "after draw-list provider failure",
        }
    }
}

impl RenderFrameOrchestrator {
    pub(in super::super) fn end_viewport_after_transient_pipeline_wait(
        controller: &mut RuntimeRenderController,
        r: &mut dyn RenderApi,
        ui_layers: Option<UiLayerDrawPacketSet>,
        scope: RenderFrameScope,
        error: impl std::fmt::Display,
    ) -> EngineResult<()> {
        log_transient_pipeline_wait_once(controller.frame.frame_index, &error.to_string());
        Self::end_degraded_viewport(
            controller,
            r,
            ui_layers,
            scope,
            DegradedViewportEnd::PipelineWait,
        )
    }

    pub(in super::super) fn end_viewport_after_pipeline_failure(
        controller: &mut RuntimeRenderController,
        r: &mut dyn RenderApi,
        ui_layers: Option<UiLayerDrawPacketSet>,
        scope: RenderFrameScope,
        error: impl std::fmt::Display,
    ) -> EngineResult<()> {
        controller
            .disable_viewport_pass("material_gpu_registry.require_primary_lit_pipeline", &error);
        Self::end_degraded_viewport(
            controller,
            r,
            ui_layers,
            scope,
            DegradedViewportEnd::PipelineFailure,
        )
    }

    pub(in super::super) fn end_viewport_after_draw_failure(
        controller: &mut RuntimeRenderController,
        r: &mut dyn RenderApi,
        ui_layers: Option<UiLayerDrawPacketSet>,
        scope: RenderFrameScope,
    ) -> EngineResult<()> {
        Self::end_degraded_viewport(
            controller,
            r,
            ui_layers,
            scope,
            DegradedViewportEnd::DrawFailure,
        )
    }

    fn end_degraded_viewport(
        controller: &mut RuntimeRenderController,
        r: &mut dyn RenderApi,
        ui_layers: Option<UiLayerDrawPacketSet>,
        scope: RenderFrameScope,
        mode: DegradedViewportEnd,
    ) -> EngineResult<()> {
        if mode.discard_commands() {
            let _ = r.discard_recorded_commands();
        }
        if mode.restore_surface_viewport() {
            r.set_viewport(Viewport::full(Extent2D::new(scope.w, scope.h)))?;
            r.set_scissor(RectI32::new(0, 0, scope.w as i32, scope.h as i32))?;
        }
        if let Some(ui_layers) = ui_layers.filter(|layers| !layers.is_empty()) {
            let envelope = build_ui_layer_frame_envelope(
                controller.frame.frame_index,
                controller.viewport.clear_color,
                Extent2D::new(scope.w, scope.h),
                ui_layers,
            );
            let _ = r.submit_frame(envelope)?;
        }
        if mode.collect_garbage() {
            controller.gc_per_draw_ubos(r);
            controller.gc_deferred_rts(r);
        }
        if scope.trace_frame {
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: end_frame frame={} {}",
                controller.frame.frame_index,
                mode.breadcrumb_suffix(),
            ));
        }
        r.end_frame()
    }
}

static TRANSIENT_SHADER_PIPELINE_WAIT_LOGGED: AtomicBool = AtomicBool::new(false);
const TRANSIENT_SHADER_PIPELINE_WAIT_HEARTBEAT_FRAMES: u64 = 240;

pub(in super::super) fn log_transient_pipeline_wait_once(frame_index: u64, error: &str) {
    if TRANSIENT_SHADER_PIPELINE_WAIT_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        newengine_ulog_api::ulog::warn!(
            "render controller: material pipeline not ready yet; shader compile remains async and viewport will retry next frame frame={} err='{}'",
            frame_index,
            error
        );
        newengine_core::crash::record_breadcrumb(format!(
            "render controller: transient material shader pipeline wait frame={} err='{}'",
            frame_index, error
        ));
    } else if frame_index.is_multiple_of(TRANSIENT_SHADER_PIPELINE_WAIT_HEARTBEAT_FRAMES) {
        newengine_ulog_api::ulog::debug!(
            "render controller: material pipeline still pending heartbeat frame={} waiting_for='renderer.shader_compile_event'",
            frame_index
        );
    }
}
