use newengine_core::host_events::CursorState;
use newengine_core::physics::PhysicsApiRef;
use newengine_core::render::{Extent2D, RenderApi};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_ui::draw::UiDrawList;
use newengine_ui_api::{UiPauseMenuState, UiRuntimeDebugOverlayTelemetry};

use super::frame_types::{PlayableFrameOutcome, RenderFrameScope, ViewportFrameInput};
use super::input::ViewportInputSnap;
use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(super) fn render_playable_viewport_frame<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        r: &mut dyn RenderApi,
        plugin_snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
    ) -> EngineResult<PlayableFrameOutcome> {
        let mut frame_input = self.read_viewport_frame_input(ctx, ui, scope);
        let pause_menu = self.menu.pause.update(
            frame_input.surface_input.as_ref(),
            &frame_input.input,
            [scope.w, scope.h],
            scope.dt,
            self.frame.frame_index,
        );
        ctx.resources_mut().insert::<UiPauseMenuState>(pause_menu.state.clone());
        if pause_menu.exit_requested {
            log::info!("pause menu: exit requested through declarative menu action");
            ctx.request_exit();
        }
        if pause_menu.blocks_gameplay {
            frame_input.input.suppress_runtime_controls();
        }

        if scope.vp_w == 0 || scope.vp_h == 0 || self.viewport.pass_disabled {
            self.render_ui_only_frame(ctx, r, frame_input.ui, scope)?;
            return Ok(PlayableFrameOutcome::Continue {
                frame_debug_snapshot: None,
            });
        }

        let extent = Extent2D::new(scope.vp_w, scope.vp_h);
        let rt = if scope.direct_surface_viewport {
            None
        } else {
            match self.ensure_viewport_rt(r, extent) {
                Ok(rt) => Some(rt),
                Err(e) => {
                    self.end_frame_after_viewport_rt_failure(r, frame_input.ui, scope, &e)?;
                    return Ok(PlayableFrameOutcome::EndedEarly { ui_telemetry: None });
                }
            }
        };

        self.bridges.scene.apply_commands();
        let scene_lock = self.bridges.scene.scene();
        let mut scene = scene_lock.write();
        let physics_api = ctx.api::<PhysicsApiRef>(newengine_core::physics::PHYSICS_API_ID).cloned();
        let job_system = ctx.job_system().cloned();
        let world_frame = self.tick_world_for_render(
            r,
            physics_api.as_ref(),
            job_system.as_ref(),
            &mut scene,
            &frame_input.input,
            frame_input.play_mode,
            scope.dt,
            pause_menu.blocks_gameplay,
            scope.aspect(),
            scope.vp_w,
            scope.vp_h,
        );

        if !world_frame.view_frame.world_playable {
            let ui_telemetry = self.end_frame_for_unplayable_world(ctx, r, &scene, frame_input.ui, scope)?;
            return Ok(PlayableFrameOutcome::EndedEarly { ui_telemetry: Some(ui_telemetry) });
        }

        if pause_menu.blocks_gameplay {
            self.sync_cursor_state(ctx, CursorState::released());
        } else {
            self.sync_cursor_state(ctx, world_frame.view_frame.cursor);
        }

        let outcome = self.submit_scene_viewport_frame(
            r,
            &scene,
            plugin_snapshot,
            frame_input.ui.as_ref(),
            frame_input.play_mode,
            rt,
            scope,
            &world_frame,
        )?;
        drop(scene);
        Ok(outcome)
    }

    fn read_viewport_frame_input<E: Send + 'static>(
        &self,
        ctx: &ModuleCtx<'_, E>,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
    ) -> ViewportFrameInput {
        let surface_input = if scope.direct_surface_viewport {
            ctx.resources().get::<newengine_ui::UiInputFrame>().cloned()
        } else {
            None
        };
        let input = if scope.direct_surface_viewport {
            ViewportInputSnap::read_direct_surface(surface_input.as_ref())
        } else {
            ViewportInputSnap::read(&self.bridges.viewport)
        };
        ViewportFrameInput {
            ui,
            input,
            surface_input,
            play_mode: self.bridges.scene.play_mode(),
        }
    }

    fn end_frame_after_viewport_rt_failure(
        &mut self,
        r: &mut dyn RenderApi,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
        error: impl std::fmt::Display,
    ) -> EngineResult<()> {
        self.disable_viewport_pass("ensure_viewport_rt", error);
        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }
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

    fn end_frame_for_unplayable_world<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        r: &mut dyn RenderApi,
        scene: &newengine_scene::Scene,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
    ) -> EngineResult<UiRuntimeDebugOverlayTelemetry> {
        let gate_reason = scene
            .world()
            .resource::<crate::gameplay::GameReadyWorldLaunchGate>()
            .map(|gate| gate.reason.clone())
            .unwrap_or_else(|| "waiting for scene launch gate".to_owned());

        self.sync_cursor_state(ctx, CursorState::released());
        let _ = r.discard_recorded_commands();
        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }
        let ui_telemetry = UiRuntimeDebugOverlayTelemetry::new(
            self.frame.frame_index,
            format!("NewEngine | Loading scene\n{}", gate_reason),
        );
        if scope.trace_frame {
            log::debug!(
                "render controller: gated loading frame={} reason='{}'",
                self.frame.frame_index,
                gate_reason
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: gated loading end_frame frame={} reason={}",
                self.frame.frame_index,
                gate_reason
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
            log::debug!(
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
