use std::sync::OnceLock;

use newengine_core::render::{
    require_render_api, BeginFrameDesc, Extent2D, RectI32, SceneLaunchStatus, Viewport,
};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_ui::draw::UiDrawList;
use newengine_ui_api::UiRuntimeDebugOverlayTelemetry;

use super::frame_types::{PlayableFrameOutcome, RenderFrameScope};
use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(super) fn render_runtime_module<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
    ) -> EngineResult<()> {
        // Do not consume the UI draw list before the native launch gate.
        // The first provider frame usually carries the font/solid atlas; if the
        // launch gate exits before a presentable frame, removing it here makes
        // subsequent atlas-free HUD frames invisible. Consume UI only when this
        // module is actually going to submit a playable/UI frame.
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
                self.viewport.clear_color = cfg.clear_color;
                cfg.work_budget
            });

        let trace_frame = super::trace_policy::should_trace_frame(self.frame.frame_index);
        let api = match require_render_api(ctx) {
            Ok(api) => api.clone(),
            Err(_) => return Ok(()),
        };
        let mut r = api.lock();
        if let Some(budget) = backend_work_budget {
            let _ = r.set_work_budget(budget);
        }

        let material_upload_jobs = backend_work_budget
            .map(|b| b.max_upload_jobs_per_frame.max(1))
            .unwrap_or(1);
        self.pump_material_texture_requests(&mut **r, material_upload_jobs, material_upload_jobs);
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
        drop(r);

        let ui: Option<UiDrawList> = ctx.resources_mut().remove::<UiDrawList>();
        let mut r = api.lock();
        let dt = ctx.frame().map(|f| f.dt).unwrap_or(0.016);
        let Some(scope) = self.begin_playable_surface_frame(&mut **r, ui.is_some(), w, h, dt, trace_frame)? else {
            drop(r);
            ctx.resources_mut().insert(SceneLaunchStatus::inactive());
            return Ok(());
        };

        self.frame.frame_index = self.frame.frame_index.saturating_add(1).max(1);
        self.gpu.meshes.instance_uploader.begin_frame();
        self.diagnostics.overlay_metrics.begin_frame(scope.dt);

        let outcome = self.render_playable_viewport_frame(
            ctx,
            &mut **r,
            plugin_snapshot.as_ref(),
            ui,
            scope,
        )?;

        let mut telemetry_to_publish = None;
        let mut ui_telemetry_to_publish = None;

        let mut frame_debug_snapshot = match outcome {
            PlayableFrameOutcome::Continue { frame_debug_snapshot } => frame_debug_snapshot,
            PlayableFrameOutcome::EndedEarly { ui_telemetry } => {
                ui_telemetry_to_publish = ui_telemetry;
                drop(r);
                ctx.resources_mut().insert(SceneLaunchStatus::inactive());
                if let Some(ui_telemetry) = ui_telemetry_to_publish {
                    ctx.resources_mut().insert(ui_telemetry);
                }
                return Ok(());
            }
        };

        {
            if let Ok(diag) = r.diagnostics_snapshot() {
                self.diagnostics.overlay_metrics.record_backend_snapshot(&diag);
                if let Some(snapshot) = frame_debug_snapshot.as_mut() {
                    snapshot.queued_upload_jobs = diag.queue.queued_upload_jobs;
                    snapshot.queued_upload_bytes = diag.queue.queued_upload_bytes;
                    snapshot.resource_buffers = diag.resources.buffers;
                    snapshot.resource_textures = diag.resources.textures;
                    snapshot.resource_pipelines = diag.resources.pipelines;
                }
            }

            self.gc_per_draw_ubos(&mut **r);
            self.gc_deferred_rts(&mut **r);
            if trace_frame {
                newengine_core::crash::record_breadcrumb(format!(
                    "render controller: end_frame frame={}",
                    self.frame.frame_index
                ));
            }
            r.end_frame()?;

            if let Some(snapshot) = frame_debug_snapshot.take() {
                self.diagnostics.overlay_metrics.publish_debug_snapshot(snapshot);
                let telemetry = self.diagnostics.overlay_metrics.telemetry_snapshot();
                if runtime_debug_overlay_enabled() {
                    let overlay_text = self.diagnostics.overlay_metrics.overlay_text();
                    let ui_telemetry = UiRuntimeDebugOverlayTelemetry::new(self.frame.frame_index, overlay_text)
                        .with_metric("render_debug", serde_json::to_value(&telemetry).unwrap_or(serde_json::Value::Null));
                    ui_telemetry_to_publish = Some(ui_telemetry);
                }
                telemetry_to_publish = Some(telemetry);
            }
            self.trace_render_diagnostics(&mut **r, trace_frame);
        }

        drop(r);
        ctx.resources_mut().insert(SceneLaunchStatus::inactive());
        if let Some(telemetry) = telemetry_to_publish {
            ctx.resources_mut().insert(telemetry);
        }
        if let Some(ui_telemetry) = ui_telemetry_to_publish {
            ctx.resources_mut().insert(ui_telemetry);
        } else {
            let _ = ctx.resources_mut().remove::<UiRuntimeDebugOverlayTelemetry>();
        }
        Ok(())
    }

    fn sync_plugin_bridge<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
    ) {
        if let Some(snap) = snapshot {
            self.bridges.plugins.publish(snap.clone());
        }
        if let Some(q) = ctx
            .resources_mut()
            .get_mut::<newengine_plugin_host::PluginControlQueue>()
        {
            for cmd in self.bridges.plugins.drain_cmds() {
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
        let (requested_vp_w, requested_vp_h) = self.bridges.viewport.read_extent();
        // UI is an overlay/service output and must not change world viewport selection.
        // A zero viewport bridge extent means "render directly to the current surface"
        // regardless of whether an engine.ui draw list is present. The previous
        // UI-gated condition turned this into 0x0 once UI provider output existed,
        // clearing the surface and drawing only UI.
        let direct_surface_viewport = requested_vp_w == 0
            && requested_vp_h == 0
            && w > 0
            && h > 0;
        let (vp_w, vp_h) = if direct_surface_viewport {
            (w, h)
        } else {
            (requested_vp_w, requested_vp_h)
        };

        self.trace_begin_frame(trace_frame, vp_w, vp_h);
        r.begin_frame(BeginFrameDesc::new(self.viewport.clear_color))?;
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

    pub(super) fn render_ui_only_frame<E: Send + 'static>(
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
        let (requested_vp_w, requested_vp_h) = self.bridges.viewport.read_extent();
        let direct_surface_viewport = requested_vp_w == 0 && requested_vp_h == 0 && w > 0 && h > 0;
        let (vp_w, vp_h) = if direct_surface_viewport {
            (w, h)
        } else {
            (requested_vp_w, requested_vp_h)
        };
        log::debug!(
            "render controller: render begin next_frame={} window={}x{} viewport={}x{} direct_surface={}",
            self.frame.frame_index.saturating_add(1),
            w,
            h,
            vp_w,
            vp_h,
            direct_surface_viewport
        );
        newengine_core::crash::record_breadcrumb(format!(
            "render controller: render begin next_frame={} window={}x{} viewport={}x{}",
            self.frame.frame_index.saturating_add(1),
            w,
            h,
            vp_w,
            vp_h
        ));
    }

    fn trace_begin_frame(&self, trace_frame: bool, vp_w: u32, vp_h: u32) {
        if !trace_frame {
            return;
        }
        log::debug!(
            "render controller: begin_frame next_frame={} clear={:.3},{:.3},{:.3},{:.3} viewport={}x{}",
            self.frame.frame_index.saturating_add(1),
            self.viewport.clear_color[0],
            self.viewport.clear_color[1],
            self.viewport.clear_color[2],
            self.viewport.clear_color[3],
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
            self.frame.frame_index.saturating_add(1)
        );
        newengine_core::crash::record_breadcrumb(format!(
            "render controller: begin_frame completed frame={}",
            self.frame.frame_index.saturating_add(1)
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

fn runtime_debug_overlay_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let configured = std::env::var("NEWENGINE_RUNTIME_DEBUG_OVERLAY").ok();
        parse_runtime_debug_overlay_setting(configured.as_deref())
    })
}

fn parse_runtime_debug_overlay_setting(value: Option<&str>) -> bool {
    match value.map(str::trim).filter(|it| !it.is_empty()) {
        // Keep the runtime statistics overlay enabled by default for the
        // GameReady/profile-dev runtime. The provider HUD is still available
        // as an explicit opt-out fallback, but the normal engine UI contract
        // should continue receiving runtime telemetry after the loading handoff.
        None => true,
        Some("0") | Some("false") | Some("FALSE") | Some("False") | Some("no")
        | Some("NO") | Some("No") | Some("off") | Some("OFF") | Some("Off") => false,
        Some("1") | Some("true") | Some("TRUE") | Some("True") | Some("yes")
        | Some("YES") | Some("Yes") | Some("on") | Some("ON") | Some("On") => true,
        Some(_) => true,
    }
}

#[cfg(test)]
mod runtime_debug_overlay_setting_tests {
    use super::parse_runtime_debug_overlay_setting;

    #[test]
    fn runtime_debug_overlay_is_enabled_by_default() {
        assert!(parse_runtime_debug_overlay_setting(None));
        assert!(parse_runtime_debug_overlay_setting(Some("")));
    }

    #[test]
    fn runtime_debug_overlay_can_be_disabled_explicitly() {
        assert!(!parse_runtime_debug_overlay_setting(Some("0")));
        assert!(!parse_runtime_debug_overlay_setting(Some("false")));
        assert!(!parse_runtime_debug_overlay_setting(Some("off")));
    }
}
