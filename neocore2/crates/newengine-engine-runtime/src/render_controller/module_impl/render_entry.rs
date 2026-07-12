use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

use crate::scene_bridge::SkyClearColorRuntime;
use newengine_core::render::{
    require_render_api, BeginFrameDesc, Extent2D, RectI32, SceneLaunchStatus, Viewport,
};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_ui_api::{UiDrawList, UiRuntimeDebugOverlayTelemetry, UiViewportSlot};

use super::super::controller::RuntimeRenderController;
use super::super::error_policy::{
    is_backend_device_lost_error, is_transient_shader_pipeline_error,
};
use super::frame_types::{PlayableFrameOutcome, RenderFrameScope};

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
                self.apply_backend_capability_profile(&cfg.capabilities);
                cfg.work_budget
            });

        let trace_frame = super::trace_policy::should_trace_frame(self.frame.frame_index);
        let api = match require_render_api(ctx) {
            Ok(api) => api.clone(),
            Err(_) => return Ok(()),
        };
        if self.backend_render_disabled() {
            ctx.resources_mut().insert(self.backend_status_snapshot());
            ctx.resources_mut().insert(SceneLaunchStatus::inactive());
            return Ok(());
        }
        let mut r = api.lock();
        if let Some(budget) = backend_work_budget {
            let _ = r.set_work_budget(budget);
        }
        self.bridge_render_backend_events(ctx, &mut **r);

        let material_upload_jobs = backend_work_budget
            .map(|b| b.max_upload_jobs_per_frame.max(1))
            .unwrap_or(1);
        let thread_pool = ctx.thread_pool().cloned();
        self.pump_material_texture_requests(
            &mut **r,
            thread_pool.as_ref(),
            material_upload_jobs,
            material_upload_jobs,
        );
        self.trace_render_begin(trace_frame, w, h);

        let prelaunch_status = match self.handle_prelaunch_gate(
            ctx,
            &mut **r,
            backend_work_budget,
            material_upload_jobs,
            trace_frame,
            w,
            h,
        ) {
            Ok(status) => status,
            Err(error) if is_backend_device_lost_error(&error) => {
                self.record_render_backend_error("render.prelaunch_gate", error)?;
                drop(r);
                ctx.resources_mut().insert(self.backend_status_snapshot());
                ctx.resources_mut().insert(SceneLaunchStatus::inactive());
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        if let Some(status) = prelaunch_status {
            drop(r);
            ctx.resources_mut().insert(status);
            return Ok(());
        }

        self.resize_if_needed(&mut **r, w, h)?;
        drop(r);

        let ui: Option<UiDrawList> = ctx.resources_mut().remove::<UiDrawList>();
        self.apply_editor_viewport_slot(ctx, w, h);
        let mut r = api.lock();
        let (dt, fixed_dt, fixed_step_count, fixed_tick) = ctx
            .frame()
            .map(|frame| {
                (
                    frame.dt,
                    frame.fixed_dt,
                    frame.fixed_step_count,
                    frame.fixed_tick,
                )
            })
            .unwrap_or((0.016, 0.016, 1, 0));
        let scope_result = self.begin_playable_surface_frame(
            &mut **r,
            ui.is_some(),
            w,
            h,
            dt,
            fixed_dt,
            fixed_step_count,
            fixed_tick,
            trace_frame,
        );
        let Some(scope) = (match scope_result {
            Ok(scope) => scope,
            Err(e) if is_backend_device_lost_error(&e) => {
                self.record_render_backend_error("render.begin_frame", e)?;
                drop(r);
                ctx.resources_mut().insert(self.backend_status_snapshot());
                ctx.resources_mut().insert(SceneLaunchStatus::inactive());
                return Ok(());
            }
            Err(e) => return Err(e),
        }) else {
            drop(r);
            ctx.resources_mut().insert(SceneLaunchStatus::inactive());
            return Ok(());
        };

        self.frame.frame_index = self.frame.frame_index.saturating_add(1).max(1);
        self.gpu.meshes.instance_uploader.begin_frame();
        self.diagnostics.overlay_metrics.begin_frame(scope.dt);

        let outcome = match catch_unwind(AssertUnwindSafe(|| {
            self.render_playable_viewport_frame(ctx, &mut **r, plugin_snapshot.as_ref(), ui, scope)
        })) {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(e)) => {
                let message = e.to_string();
                if is_backend_device_lost_error(&e) {
                    self.record_render_backend_error("render.playable_frame.error", e)?;
                    drop(r);
                    ctx.resources_mut().insert(self.backend_status_snapshot());
                    ctx.resources_mut().insert(SceneLaunchStatus::inactive());
                    return Ok(());
                }
                if is_transient_shader_pipeline_error(&e) {
                    newengine_ulog_api::ulog::warn!(
                        "render controller: playable frame yielded while shader pipeline is pending; keeping viewport pass retryable: {}",
                        message
                    );
                    let _ = r.discard_recorded_commands();
                    let _ = r.end_frame();
                    drop(r);
                    ctx.resources_mut().insert(SceneLaunchStatus::inactive());
                    return Ok(());
                }
                self.disable_viewport_pass("render_playable_viewport_frame.error", &message);
                newengine_ulog_api::ulog::error!(
                    "render controller: playable frame returned error; presenting degraded recovery frame instead of aborting: {}",
                    message
                );
                let _ = r.discard_recorded_commands();
                let _ = r.end_frame();
                drop(r);
                ctx.resources_mut()
                    .insert(newengine_core::render::RenderBackendStatus::degraded(
                        "render.playable_frame.error",
                        message,
                    ));
                ctx.resources_mut().insert(SceneLaunchStatus::inactive());
                return Ok(());
            }
            Err(payload) => {
                let message = panic_payload_message(payload);
                self.disable_viewport_pass("render_playable_viewport_frame.panic", &message);
                newengine_ulog_api::ulog::error!(
                    "render controller: caught panic during playable frame; presenting degraded recovery frame instead of aborting: {}",
                    message
                );
                newengine_core::crash::record_breadcrumb(format!(
                    "render controller: caught playable-frame panic frame={} msg='{}'",
                    self.frame.frame_index, message
                ));
                let _ = r.discard_recorded_commands();
                let _ = r.end_frame();
                drop(r);
                ctx.resources_mut()
                    .insert(newengine_core::render::RenderBackendStatus::degraded(
                        "render.playable_frame.panic",
                        message,
                    ));
                ctx.resources_mut().insert(SceneLaunchStatus::inactive());
                return Ok(());
            }
        };

        let mut telemetry_to_publish = None;
        let mut ui_telemetry_to_publish = None;

        let mut frame_debug_snapshot = match outcome {
            PlayableFrameOutcome::Continue {
                frame_debug_snapshot,
            } => frame_debug_snapshot,
            PlayableFrameOutcome::EndedEarly { ui_telemetry } => {
                ui_telemetry_to_publish = ui_telemetry;
                drop(r);
                if self.backend_render_disabled() {
                    ctx.resources_mut().insert(self.backend_status_snapshot());
                }
                ctx.resources_mut().insert(SceneLaunchStatus::inactive());
                if let Some(ui_telemetry) = ui_telemetry_to_publish {
                    ctx.resources_mut().insert(ui_telemetry);
                }
                return Ok(());
            }
        };

        {
            if let Ok(diag) = r.diagnostics_snapshot() {
                self.diagnostics
                    .overlay_metrics
                    .record_backend_snapshot(&diag);
                if let Some(snapshot) = frame_debug_snapshot.as_mut() {
                    snapshot.queued_upload_jobs = diag.queue.queued_upload_jobs;
                    snapshot.queued_upload_bytes = diag.queue.queued_upload_bytes;
                    snapshot.resource_buffers = diag.resources.buffers;
                    snapshot.resource_textures = diag.resources.textures;
                    snapshot.resource_pipelines = diag.resources.pipelines;
                }
            }

            if trace_frame {
                newengine_core::crash::record_breadcrumb(format!(
                    "render controller: end_frame frame={}",
                    self.frame.frame_index
                ));
            }
            if let Err(e) = r.end_frame() {
                if is_backend_device_lost_error(&e) {
                    self.record_render_backend_error("render.end_frame", e)?;
                    drop(r);
                    ctx.resources_mut().insert(self.backend_status_snapshot());
                    ctx.resources_mut().insert(SceneLaunchStatus::inactive());
                    return Ok(());
                }
                return Err(e);
            }
            self.bridge_render_backend_events(ctx, &mut **r);

            if let Some(snapshot) = frame_debug_snapshot.take() {
                self.diagnostics
                    .overlay_metrics
                    .publish_debug_snapshot(snapshot);
                let telemetry = self.diagnostics.overlay_metrics.telemetry_snapshot();
                if runtime_debug_overlay_enabled() {
                    let overlay_text = self.diagnostics.overlay_metrics.overlay_text();
                    let ui_telemetry =
                        UiRuntimeDebugOverlayTelemetry::new(self.frame.frame_index, overlay_text)
                            .with_metric(
                                "render_debug",
                                serde_json::to_value(&telemetry).unwrap_or(serde_json::Value::Null),
                            );
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
            let _ = ctx
                .resources_mut()
                .remove::<UiRuntimeDebugOverlayTelemetry>();
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

    fn apply_editor_viewport_slot<E: Send + 'static>(
        &self,
        ctx: &ModuleCtx<'_, E>,
        surface_w: u32,
        surface_h: u32,
    ) {
        let Some(slot) = ctx.resources().get::<UiViewportSlot>() else {
            return;
        };
        let (mut vp_w, mut vp_h) = slot.extent_px();
        if vp_w == 0 || vp_h == 0 {
            self.bridges.viewport.publish_extent(0, 0);
            return;
        }
        vp_w = vp_w.min(surface_w.max(1));
        vp_h = vp_h.min(surface_h.max(1));
        self.bridges.viewport.publish_extent(vp_w, vp_h);
    }

    fn begin_playable_surface_frame(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        ui_enabled: bool,
        w: u32,
        h: u32,
        dt: f32,
        fixed_dt: f32,
        fixed_step_count: u32,
        fixed_tick: u64,
        trace_frame: bool,
    ) -> EngineResult<Option<RenderFrameScope>> {
        let (requested_vp_w, requested_vp_h) = self.bridges.viewport.read_extent();
        // UI is an overlay/service output and must not change world viewport selection.
        // A zero viewport bridge extent means "render directly to the current surface"
        // regardless of whether an engine.ui draw list is present. The previous
        // UI-gated condition turned this into 0x0 once UI provider output existed,
        // clearing the surface and drawing only UI.
        let direct_surface_viewport = requested_vp_w == 0 && requested_vp_h == 0 && w > 0 && h > 0;
        let (vp_w, vp_h) = if direct_surface_viewport {
            (w, h)
        } else {
            (requested_vp_w, requested_vp_h)
        };

        self.viewport.clear_color = self
            .bridges
            .scene
            .scene()
            .read()
            .world()
            .resource::<SkyClearColorRuntime>()
            .map(|sky| sky.color)
            .unwrap_or_else(|| self.runtime_profile().configured_clear_color());
        self.trace_begin_frame(trace_frame, vp_w, vp_h);
        r.begin_frame(
            BeginFrameDesc::new(self.viewport.clear_color)
                .with_frame_index(self.frame.frame_index.saturating_add(1).max(1)),
        )?;
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
            fixed_dt,
            fixed_step_count,
            fixed_tick,
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
        newengine_ulog_api::ulog::debug!(
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
        newengine_ulog_api::ulog::debug!(
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
        newengine_ulog_api::ulog::debug!(
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
            newengine_ulog_api::ulog::debug!(
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

fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        (*msg).to_owned()
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.clone()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

fn runtime_debug_overlay_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let configured = crate::env_config::var("NEWENGINE_RUNTIME_DEBUG_OVERLAY");
        parse_runtime_debug_overlay_setting(configured.as_deref())
    })
}

fn parse_runtime_debug_overlay_setting(value: Option<&str>) -> bool {
    match value.map(str::trim).filter(|it| !it.is_empty()) {
        // Default game viewport should be a clean HUD-only surface. Enable this
        // explicitly with NEWENGINE_RUNTIME_DEBUG_OVERLAY=1 when diagnosing frame
        // metrics; otherwise the retained debug surface churns the UI every frame.
        None => false,
        Some("0") | Some("false") | Some("FALSE") | Some("False") | Some("no") | Some("NO")
        | Some("No") | Some("off") | Some("OFF") | Some("Off") => false,
        Some("1") | Some("true") | Some("TRUE") | Some("True") | Some("yes") | Some("YES")
        | Some("Yes") | Some("on") | Some("ON") | Some("On") => true,
        Some(_) => true,
    }
}

#[cfg(test)]
mod runtime_debug_overlay_setting_tests {
    use super::parse_runtime_debug_overlay_setting;

    #[test]
    fn runtime_debug_overlay_is_disabled_by_default() {
        assert!(!parse_runtime_debug_overlay_setting(None));
        assert!(!parse_runtime_debug_overlay_setting(Some("")));
    }

    #[test]
    fn runtime_debug_overlay_can_be_disabled_explicitly() {
        assert!(!parse_runtime_debug_overlay_setting(Some("0")));
        assert!(!parse_runtime_debug_overlay_setting(Some("false")));
        assert!(!parse_runtime_debug_overlay_setting(Some("off")));
    }
}
