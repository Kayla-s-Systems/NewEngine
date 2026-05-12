use newengine_camera_runtime::cursor_state_for_nav;
use newengine_core::host_events::CursorState;
use newengine_core::render::{Extent2D, RenderApi};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_ui::draw::UiDrawList;

use super::frame_types::{PlayableFrameOutcome, RenderFrameScope, ViewportFrameInput};
use super::input::ViewportInputSnap;
use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(super) fn render_playable_viewport_frame<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        r: &mut dyn RenderApi,
        plugin_snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
    ) -> EngineResult<PlayableFrameOutcome> {
        if scope.vp_w == 0 || scope.vp_h == 0 || self.viewport_pass_disabled {
            self.render_ui_only_frame(ctx, r, ui, scope)?;
            return Ok(PlayableFrameOutcome::Continue {
                frame_debug_snapshot: None,
            });
        }

        let frame_input = self.read_viewport_frame_input(ctx, ui, scope);
        let extent = Extent2D::new(scope.vp_w, scope.vp_h);
        let rt = if scope.direct_surface_viewport {
            None
        } else {
            match self.ensure_viewport_rt(r, extent) {
                Ok(rt) => Some(rt),
                Err(e) => {
                    self.end_frame_after_viewport_rt_failure(r, frame_input.ui, scope, &e)?;
                    return Ok(PlayableFrameOutcome::EndedEarly);
                }
            }
        };

        self.scene_bridge.apply_commands();
        let scene_lock = self.scene_bridge.scene();
        let mut scene = scene_lock.write();
        let world_frame = self.tick_world_for_render(
            r,
            &mut scene,
            &frame_input.input,
            frame_input.play_mode,
            scope.dt,
            scope.aspect(),
            scope.vp_w,
            scope.vp_h,
        );

        if !world_frame.world_playable {
            self.end_frame_for_unplayable_world(ctx, r, &scene, frame_input.ui, scope)?;
            return Ok(PlayableFrameOutcome::EndedEarly);
        }

        let desired_cursor = if world_frame.effective_play_mode.wants_direct_player_control()
            && frame_input.input.active
        {
            CursorState::captured_locked()
        } else {
            cursor_state_for_nav(&world_frame.nav_input)
        };
        self.sync_cursor_state(ctx, desired_cursor);

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
        let input = if scope.direct_surface_viewport {
            ViewportInputSnap::read_direct_surface(ctx.resources().get::<newengine_ui::UiInputFrame>())
        } else {
            ViewportInputSnap::read(&self.viewport_bridge)
        };
        ViewportFrameInput {
            ui,
            input,
            play_mode: self.scene_bridge.play_mode(),
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
                self.frame_index
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
    ) -> EngineResult<()> {
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
        r.set_debug_text(format!("NewEngine | Loading scene\n{}", gate_reason));
        if scope.trace_frame {
            log::debug!(
                "render controller: gated loading frame={} reason='{}'",
                self.frame_index,
                gate_reason
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: gated loading end_frame frame={} reason={}",
                self.frame_index,
                gate_reason
            ));
        }
        r.end_frame()?;
        self.trace_gated_diagnostics(r, scope.trace_frame);
        Ok(())
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
