use newengine_core::render::{
    require_render_api, BeginFrameDesc, Extent2D, RectI32, SceneLaunchStatus, Viewport,
};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_ui::draw::UiDrawList;

use super::frame_types::{PlayableFrameOutcome, RenderFrameScope};
use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(super) fn render_runtime_module<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
    ) -> EngineResult<()> {
        let ui: Option<UiDrawList> = ctx.resources_mut().remove::<UiDrawList>();
        let plugin_snapshot = ctx
            .resources()
            .get::<newengine_plugin_host::PluginsSnapshot>()
            .cloned();
        self.sync_plugin_bridge(ctx, plugin_snapshot.as_ref());

        let (w, h) = Self::read_window_size(ctx);
        let backend_work_budget = ctx
            .resources()
            .get::<crate::render_runtime::ResolvedRenderBackendConfig>()
            .map(|cfg| {
                self.clear_color = cfg.clear_color;
                cfg.work_budget
            });

        let trace_frame = super::trace_policy::should_trace_frame(self.frame_index);
        let api = match require_render_api(ctx) {
            Ok(api) => api,
            Err(_) => return Ok(()),
        };
        let mut r = api.lock();
        if let Some(budget) = backend_work_budget {
            let _ = r.set_work_budget(budget);
        }

        let material_upload_jobs = backend_work_budget
            .map(|b| b.max_upload_jobs_per_frame.max(1))
            .unwrap_or(1);
        self.pump_material_texture_requests(&mut **r, material_upload_jobs);
        self.trace_render_begin(trace_frame, w, h);

        if let Some(status) = self.handle_native_prelaunch_gate(
            ctx,
            &mut **r,
            backend_work_budget,
            material_upload_jobs,
            trace_frame,
        )? {
            drop(r);
            ctx.resources_mut().insert(status);
            return Ok(());
        }

        self.resize_if_needed(&mut **r, w, h)?;
        let dt = ctx.frame().map(|f| f.dt).unwrap_or(0.016);
        let Some(scope) = self.begin_playable_surface_frame(&mut **r, ui.is_some(), w, h, dt, trace_frame)? else {
            drop(r);
            ctx.resources_mut().insert(SceneLaunchStatus::inactive());
            return Ok(());
        };

        self.frame_index = self.frame_index.saturating_add(1).max(1);
        self.overlay_metrics.begin_frame(scope.dt);
        self.pump_previews_fail_soft(&mut **r, scope.dt);

        let outcome = self.render_playable_viewport_frame(
            ctx,
            &mut **r,
            plugin_snapshot.as_ref(),
            ui,
            scope,
        )?;

        if matches!(outcome, PlayableFrameOutcome::EndedEarly) {
            drop(r);
            ctx.resources_mut().insert(SceneLaunchStatus::inactive());
            return Ok(());
        }

        let mut telemetry_to_publish = None;
        if let PlayableFrameOutcome::Continue {
            mut frame_debug_snapshot,
        } = outcome
        {
            if let Ok(diag) = r.diagnostics_snapshot() {
                self.overlay_metrics.record_backend_snapshot(&diag);
                if let Some(snapshot) = frame_debug_snapshot.as_mut() {
                    snapshot.queued_upload_jobs = diag.queue.queued_upload_jobs;
                    snapshot.queued_upload_bytes = diag.queue.queued_upload_bytes;
                    snapshot.resource_buffers = diag.resources.buffers;
                    snapshot.resource_textures = diag.resources.textures;
                    snapshot.resource_pipelines = diag.resources.pipelines;
                }
            }

            r.set_debug_text(self.overlay_metrics.overlay_text());
            self.gc_per_draw_ubos(&mut **r);
            self.gc_deferred_rts(&mut **r);
            if trace_frame {
                newengine_core::crash::record_breadcrumb(format!(
                    "render controller: end_frame frame={}",
                    self.frame_index
                ));
            }
            r.end_frame()?;

            if let Some(snapshot) = frame_debug_snapshot.take() {
                self.overlay_metrics.publish_debug_snapshot(snapshot);
                telemetry_to_publish = Some(self.overlay_metrics.telemetry_snapshot());
            }
            self.trace_render_diagnostics(&mut **r, trace_frame);
        }

        drop(r);
        ctx.resources_mut().insert(SceneLaunchStatus::inactive());
        if let Some(telemetry) = telemetry_to_publish {
            ctx.resources_mut().insert(telemetry);
        }
        Ok(())
    }

    fn sync_plugin_bridge<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
    ) {
        if let Some(snap) = snapshot {
            self.plugins_bridge.publish(snap.clone());
        }
        if let Some(q) = ctx
            .resources_mut()
            .get_mut::<newengine_plugin_host::PluginControlQueue>()
        {
            for cmd in self.plugins_bridge.drain_cmds() {
                q.push(cmd);
            }
        }
    }

    fn begin_playable_surface_frame(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        ui_enabled: bool,
        w: u32,
        h: u32,
        dt: f32,
        trace_frame: bool,
    ) -> EngineResult<Option<RenderFrameScope>> {
        let (requested_vp_w, requested_vp_h) = self.viewport_bridge.read_extent();
        let direct_surface_viewport = !ui_enabled
            && requested_vp_w == 0
            && requested_vp_h == 0
            && w > 0
            && h > 0;
        let (vp_w, vp_h) = if direct_surface_viewport {
            (w, h)
        } else {
            (requested_vp_w, requested_vp_h)
        };

        self.trace_begin_frame(trace_frame, vp_w, vp_h);
        r.begin_frame(BeginFrameDesc::new(self.clear_color))?;
        self.trace_begin_frame_done(trace_frame);

        Ok(Some(RenderFrameScope {
            w,
            h,
            vp_w,
            vp_h,
            direct_surface_viewport,
            ui_enabled,
            trace_frame,
            dt,
        }))
    }

    pub(super) fn render_legacy_ui_only_frame<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        r: &mut dyn newengine_core::render::RenderApi,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
    ) -> EngineResult<()> {
        self.sync_cursor_state(ctx, newengine_core::host_events::CursorState::released());
        if let Some(ui) = ui {
            let win_extent = Extent2D::new(scope.w, scope.h);
            r.set_viewport(Viewport::full(win_extent))?;
            r.set_scissor(RectI32::new(0, 0, scope.w as i32, scope.h as i32))?;
            r.set_ui_draw_list(ui);
        }
        Ok(())
    }

    fn trace_render_begin(&self, trace_frame: bool, w: u32, h: u32) {
        if !trace_frame {
            return;
        }
        let (vp_w, vp_h) = self.viewport_bridge.read_extent();
        log::debug!(
            "render controller: render begin next_frame={} window={}x{} viewport={}x{}",
            self.frame_index.saturating_add(1),
            w,
            h,
            vp_w,
            vp_h
        );
        newengine_core::crash::record_breadcrumb(format!(
            "render controller: render begin next_frame={} window={}x{}",
            self.frame_index.saturating_add(1),
            w,
            h
        ));
    }

    fn trace_begin_frame(&self, trace_frame: bool, vp_w: u32, vp_h: u32) {
        if !trace_frame {
            return;
        }
        log::debug!(
            "render controller: begin_frame next_frame={} clear={:.3},{:.3},{:.3},{:.3} viewport={}x{}",
            self.frame_index.saturating_add(1),
            self.clear_color[0],
            self.clear_color[1],
            self.clear_color[2],
            self.clear_color[3],
            vp_w,
            vp_h
        );
    }

    fn trace_begin_frame_done(&self, trace_frame: bool) {
        if !trace_frame {
            return;
        }
        log::debug!(
            "render controller: begin_frame completed frame={}",
            self.frame_index.saturating_add(1)
        );
        newengine_core::crash::record_breadcrumb(format!(
            "render controller: begin_frame completed frame={}",
            self.frame_index.saturating_add(1)
        ));
    }

    fn trace_render_diagnostics(
        &self,
        r: &mut dyn newengine_core::render::RenderApi,
        trace_frame: bool,
    ) {
        if !trace_frame {
            return;
        }
        if let Ok(diag) = r.diagnostics_snapshot() {
            log::debug!(
                "render diagnostics: frame={} begin_ms={:.3} end_ms={:.3} upload_ms={:.3} pipeline_ms={:.3} buffers={} textures={} pipelines={} upload_jobs={} upload_mb={:.2} queued_uploads={} queued_mb={:.2}",
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
