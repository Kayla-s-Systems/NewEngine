use super::*;

impl RuntimeRenderController {
    pub(super) fn end_frame_after_viewport_rt_failure<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        r: &mut dyn RenderApi,
        ui_layers: UiLayerDrawPacketSet,
        scope: RenderFrameScope,
        error: impl std::fmt::Display,
    ) -> EngineResult<()> {
        self.disable_viewport_pass("ensure_viewport_rt", error);
        self.render_ui_only_frame(ctx, r, ui_layers, scope)?;
        self.gc_per_draw_ubos(r);
        self.gc_deferred_rts(r);
        if scope.trace_frame {
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: end_frame frame={} after viewport RT failure",
                self.frame.frame_index
            ));
        }
        r.end_frame()
    }

    pub(super) fn end_frame_for_unplayable_world<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        r: &mut dyn RenderApi,
        scene: &newengine_scene::Scene,
        ui_layers: UiLayerDrawPacketSet,
        scope: RenderFrameScope,
    ) -> EngineResult<UiRuntimeDebugOverlayTelemetry> {
        let gate_reason = scene
            .world()
            .resource::<crate::gameplay::WorldActivationState>()
            .map(|gate| gate.reason.clone())
            .unwrap_or_else(|| "waiting for scene launch gate".to_owned());

        self.sync_cursor_state(ctx, CursorState::released());
        let _ = r.discard_recorded_commands();
        // Gated world frames still present through the normal typed layer-packet envelope.
        self.render_ui_only_frame(ctx, r, ui_layers, scope)?;
        let ui_telemetry = UiRuntimeDebugOverlayTelemetry::new(
            self.frame.frame_index,
            format!("NewEngine | Loading scene\n{}", gate_reason),
        );
        if scope.trace_frame {
            newengine_ulog_api::ulog::debug!(
                "render controller: gated loading frame={} reason='{}'",
                self.frame.frame_index,
                gate_reason
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: gated loading end_frame frame={} reason={}",
                self.frame.frame_index, gate_reason
            ));
        }
        r.end_frame()?;
        self.trace_gated_diagnostics(r, scope.trace_frame);
        Ok(ui_telemetry)
    }

    fn trace_gated_diagnostics(&self, r: &mut dyn RenderApi, trace_frame: bool) {
        if !trace_frame {
            return;
        }
        if let Ok(diag) = r.diagnostics_snapshot() {
            newengine_ulog_api::ulog::debug!(
                "render diagnostics: frame={} gated_loading=true begin_ms={:.3} end_ms={:.3} upload_ms={:.3} pipeline_ms={:.3} buffers={} textures={} pipelines={} upload_jobs={} upload_mb={:.2} queued_uploads={} queued_mb={:.2}",
                diag.frame.frame_index,
                diag.frame.last_begin_frame_ms,
                diag.frame.last_end_frame_ms,
                diag.frame.last_blocking_upload_ms,
                diag.frame.last_pipeline_build_ms,
                diag.resources.buffers,
                diag.resources.textures,
                diag.resources.pipelines,
                diag.queue.blocking_upload_jobs,
                diag.queue.blocking_upload_bytes as f32 / (1024.0 * 1024.0),
                diag.queue.queued_upload_jobs,
                diag.queue.queued_upload_bytes as f32 / (1024.0 * 1024.0),
            );
        }
    }
}
