use newengine_core::host_events::CursorState;
use newengine_core::physics::PhysicsApiRef;
use newengine_core::render::{Extent2D, RenderApi};
use newengine_core::{EngineResult, ModuleCtx};
use crate::ui_gateway;
use crate::input_systems::InputCaptureState;
use newengine_ui_api::{UiDrawList, UiInputCaptureState, UiRuntimeDebugOverlayTelemetry, UiSurfaceNode};

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
        let primary_was_open = self.ui.primary.is_open();
        let primary_ui = self.ui.primary.update(
            frame_input.surface_input.as_ref(),
            &frame_input.input,
            [scope.w, scope.h],
            scope.dt,
            self.frame.frame_index,
        );
        let external_ui_capture = ctx
            .resources()
            .get::<UiInputCaptureState>()
            .cloned()
            .unwrap_or_else(UiInputCaptureState::none);
        let modal_blocks_gameplay = primary_ui.blocks_gameplay || external_ui_capture.requests_capture();

        self.refresh_modal_ui_draw_list(
            ctx,
            &mut frame_input.ui,
            &primary_ui.state,
            primary_was_open,
            &external_ui_capture,
            scope,
        )?;
        if primary_was_open && !primary_ui.blocks_gameplay {
            self.restore_playable_view_after_ui_close();
        }
        if primary_ui.exit_requested {
            log::info!("UI surface: exit requested through declarative menu action");
            ctx.request_exit();
        }
        {
            let mut carrier = frame_input.input.action_carrier();
            let published_capture = external_ui_capture.merged_with_primary_modal(primary_ui.blocks_gameplay);
            self.frame.input_systems.publish_input_capture_state(
                self.frame.frame_index,
                InputCaptureState::modal_ui(published_capture.requests_capture()),
                &mut carrier,
            );
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
            Some(ctx.events()),
            &mut scene,
            &frame_input.input,
            frame_input.play_mode,
            scope.dt,
            modal_blocks_gameplay,
            scope.aspect(),
            scope.vp_w,
            scope.vp_h,
        );

        if !world_frame.view_frame.world_playable {
            let ui_telemetry = self.end_frame_for_unplayable_world(ctx, r, &scene, frame_input.ui, scope)?;
            return Ok(PlayableFrameOutcome::EndedEarly { ui_telemetry: Some(ui_telemetry) });
        }

        if modal_blocks_gameplay {
            // Modal UI must visibly release the OS cursor even if runtime-side
            // state already believes it is released. Platform grabs can be lost
            // or retained across focus/UI transitions, so force a release event.
            self.force_cursor_state(ctx, CursorState::released());
        } else if self.runtime_profile().input.capture_cursor_on_play {
            self.sync_cursor_state(ctx, world_frame.view_frame.cursor);
        } else {
            self.sync_cursor_state(ctx, CursorState::released());
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


    fn refresh_modal_ui_draw_list<E: Send + 'static>(
        &self,
        _ctx: &ModuleCtx<'_, E>,
        ui: &mut Option<UiDrawList>,
        primary_state: &UiSurfaceNode,
        primary_was_open: bool,
        external_capture: &UiInputCaptureState,
        scope: RenderFrameScope,
    ) -> EngineResult<()> {
        if primary_state.visible || primary_was_open {
            // Publish both visible and hidden states. engine.ui owns retained node
            // lifecycle; if runtime does not send the hidden node on close, the
            // provider can legally keep the previous retained menu on screen.
            ui_gateway::publish_surface_node(primary_state);
        }

        let external_refresh = external_capture.draw_refresh_requested || external_capture.requests_capture();

        if !primary_state.visible && !primary_was_open && !external_refresh {
            return Ok(());
        }

        let needs_clear_packet = (!primary_state.visible && primary_was_open)
            || (external_capture.draw_refresh_requested && !external_capture.requests_capture());

        match ui_gateway::request_draw_list(
            self.frame.frame_index,
            scope.dt,
            [scope.w, scope.h],
            1.0,
        ) {
            Ok(Some(draw_list)) => {
                *ui = Some(draw_list);
            }
            Ok(None) => {
                if needs_clear_packet {
                    *ui = Some(clear_ui_draw_list([scope.w, scope.h]));
                }
            }
            Err(e) => {
                log::warn!("modal ui: same-frame draw-list refresh failed: {e}");
                if needs_clear_packet {
                    *ui = Some(clear_ui_draw_list([scope.w, scope.h]));
                }
            }
        }

        Ok(())
    }

    fn read_viewport_frame_input<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
    ) -> ViewportFrameInput {
        let surface_input = ctx.resources().get::<newengine_ui_api::UiInputFrame>().cloned();
        let mut input = if scope.direct_surface_viewport {
            ViewportInputSnap::read_direct_surface(surface_input.as_ref())
        } else {
            let mut input = ViewportInputSnap::read(&self.bridges.viewport);
            input.merge_semantic_actions_from_surface(surface_input.as_ref());
            input
        };
        {
            let mut carrier = input.action_carrier();
            self.frame.input_systems.observe_frame(
                self.frame.frame_index,
                surface_input.as_ref(),
                &mut carrier,
            );
        }
        let play_mode = self.bridges.scene.play_mode();
        if play_mode.wants_direct_player_control() {
            input.apply_gameplay_input_handoff(&self.runtime_profile().input);
        }
        ViewportFrameInput {
            ui,
            input,
            surface_input,
            play_mode,
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


fn clear_ui_draw_list(surface_size_px: [u32; 2]) -> UiDrawList {
    let mut draw_list = UiDrawList::new();
    draw_list.screen_size_px = surface_size_px;
    draw_list.pixels_per_point = 1.0;
    draw_list
}
