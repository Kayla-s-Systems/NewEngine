use crate::input_systems::InputCaptureState;
use crate::ui_gateway;
use newengine_core::host_events::CursorState;
use newengine_core::physics::PhysicsApiRef;
use newengine_core::render::{Extent2D, RenderApi};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_ui_api::{
    UiDrawCmd, UiDrawList, UiEditorRuntimeMode, UiEditorRuntimeState, UiInputCaptureState, UiRect,
    UiRuntimeDebugOverlayTelemetry, UiScreenProfile, UiScreenProfileState, UiSurfaceNode, UiTexId,
    UiVertex, UiViewportSlot,
};

use super::super::controller::RuntimeRenderController;
use super::frame_types::{PlayableFrameOutcome, RenderFrameScope, ViewportFrameInput};
use super::input::ViewportInputSnap;

#[inline]
fn viewport_texture_color() -> u32 {
    255 | (255 << 8) | (255 << 16) | (255 << 24)
}

fn prepend_viewport_slot_quad(ui: &mut UiDrawList, slot: &UiViewportSlot, texture_id: u32) {
    if texture_id == 0 || slot.w_px <= 1.0 || slot.h_px <= 1.0 {
        return;
    }
    let old_vertices = core::mem::take(&mut ui.mesh.vertices);
    let old_indices = core::mem::take(&mut ui.mesh.indices);
    let old_cmds: Vec<UiDrawCmd> = ui.mesh.cmds.drain(..).collect();

    let x = slot.x_px.round();
    let y = slot.y_px.round();
    let w = slot.w_px.round().max(1.0);
    let h = slot.h_px.round().max(1.0);
    let color = viewport_texture_color();
    ui.mesh.vertices.extend_from_slice(&[
        UiVertex {
            pos: [x, y],
            uv: [0.0, 0.0],
            color,
        },
        UiVertex {
            pos: [x + w, y],
            uv: [1.0, 0.0],
            color,
        },
        UiVertex {
            pos: [x + w, y + h],
            uv: [1.0, 1.0],
            color,
        },
        UiVertex {
            pos: [x, y + h],
            uv: [0.0, 1.0],
            color,
        },
    ]);
    ui.mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    ui.mesh.cmds.push(UiDrawCmd {
        texture: UiTexId(texture_id),
        clip_rect: UiRect {
            min_x: x,
            min_y: y,
            max_x: x + w,
            max_y: y + h,
        },
        index_range: 0..6,
    });

    let vertex_offset = 4u32;
    let index_offset = 6u32;
    ui.mesh.vertices.extend(old_vertices);
    ui.mesh
        .indices
        .extend(old_indices.into_iter().map(|idx| idx + vertex_offset));
    for mut cmd in old_cmds {
        cmd.index_range =
            (cmd.index_range.start + index_offset)..(cmd.index_range.end + index_offset);
        ui.mesh.cmds.push(cmd);
    }
}

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
        let game_profile = is_game_screen_profile(ctx);
        if game_profile
            && frame_input
                .surface_input
                .as_ref()
                .is_some_and(|input| input.is_key_pressed(newengine_input_api::key_code::F2))
        {
            self.bridges.scene.toggle_in_game_editor();
        } else if !game_profile && self.bridges.scene.in_game_editor_enabled() {
            self.bridges.scene.set_in_game_editor_enabled(false);
        }

        if let Some(dispatch_frame) = ctx
            .resources()
            .get::<newengine_ui_api::UiEventDispatchFrame>()
        {
            if game_profile {
                let _ = self
                    .bridges
                    .scene
                    .apply_in_game_editor_actions(dispatch_frame);
            }
            let _ = self
                .bridges
                .scene
                .apply_editor_selection_actions(dispatch_frame);
            let _ = self
                .bridges
                .scene
                .apply_inventory_ui_actions(dispatch_frame);
        }
        let in_game_editor = game_profile && self.bridges.scene.in_game_editor_enabled();
        if in_game_editor && scope.vp_w > 0 && scope.vp_h > 0 {
            self.bridges.viewport.publish_pick_request(
                (scope.vp_w.saturating_sub(1) as f32) * 0.5,
                (scope.vp_h.saturating_sub(1) as f32) * 0.5,
            );
        }

        let external_ui_capture = ctx
            .resources()
            .get::<UiInputCaptureState>()
            .cloned()
            .unwrap_or_else(UiInputCaptureState::none);
        let provider_ui_capture = self.refresh_modal_ui_draw_list(
            ctx,
            &mut frame_input.ui,
            &primary_ui.state,
            primary_was_open,
            &external_ui_capture,
            scope,
        )?;
        let published_capture = merge_ui_input_capture(
            external_ui_capture.merged_with_primary_modal(primary_ui.blocks_gameplay),
            provider_ui_capture.unwrap_or_else(UiInputCaptureState::none),
        );
        let modal_blocks_gameplay = published_capture.requests_capture() || in_game_editor;
        if primary_was_open && !primary_ui.blocks_gameplay {
            self.restore_playable_view_after_ui_close();
        }
        if primary_ui.exit_requested {
            newengine_ulog_api::ulog::info!(
                "UI surface: exit requested through declarative menu action"
            );
            ctx.request_exit();
        }
        {
            let mut carrier = frame_input.input.action_carrier();
            self.frame.input_systems.publish_input_capture_state(
                self.frame.frame_index,
                InputCaptureState::modal_ui(published_capture.requests_capture()),
                &mut carrier,
            );
        }

        if editor_viewport_runtime_mode(ctx) == Some(UiEditorRuntimeMode::Edit) {
            // Editor/Edit is a tooling state, not a playable-world frame. Keep the
            // viewport slot as UI chrome only and do not tick scene/world, build
            // game pipelines, run shadow planning, or submit gameplay draw-list
            // providers. Simulate/Play explicitly re-enable the world path.
            self.render_ui_only_frame(ctx, r, frame_input.ui, scope)?;
            return Ok(PlayableFrameOutcome::Continue {
                frame_debug_snapshot: None,
            });
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

        if rt.is_some() {
            let slot = ctx.resources().get::<UiViewportSlot>().cloned();
            let viewport_tex = self.bridges.viewport.read_tex_user() as u32;
            if let (Some(slot), Some(ui)) = (slot.as_ref(), frame_input.ui.as_mut()) {
                prepend_viewport_slot_quad(ui, slot, viewport_tex);
            }
        }

        self.bridges.scene.apply_commands();
        let scene_lock = self.bridges.scene.scene();
        let mut scene = scene_lock.write();
        let physics_api = ctx
            .api::<PhysicsApiRef>(newengine_core::physics::PHYSICS_API_ID)
            .cloned();
        let thread_pool = ctx.thread_pool().cloned();
        let world_frame = self.tick_world_for_render(
            r,
            physics_api.as_ref(),
            thread_pool.as_ref(),
            Some(ctx.events()),
            &mut scene,
            &frame_input.input,
            frame_input.play_mode,
            scope.dt,
            scope.fixed_dt,
            scope.fixed_step_count,
            scope.fixed_tick,
            modal_blocks_gameplay,
            scope.aspect(),
            scope.vp_w,
            scope.vp_h,
        );
        crate::gameplay::publish_inventory_hud_state(scene.world_mut(), self.frame.frame_index);

        if !world_frame.view_frame.world_playable {
            let ui_telemetry =
                self.end_frame_for_unplayable_world(ctx, r, &scene, frame_input.ui, scope)?;
            return Ok(PlayableFrameOutcome::EndedEarly {
                ui_telemetry: Some(ui_telemetry),
            });
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
            thread_pool.as_ref(),
        );
        drop(scene);
        if let Some(picked) = self.frame.pending_pick_selection.take() {
            self.bridges.scene.set_selection(picked);
        }
        Ok(outcome?)
    }

    fn refresh_modal_ui_draw_list<E: Send + 'static>(
        &self,
        _ctx: &ModuleCtx<'_, E>,
        ui: &mut Option<UiDrawList>,
        primary_state: &UiSurfaceNode,
        primary_was_open: bool,
        external_capture: &UiInputCaptureState,
        scope: RenderFrameScope,
    ) -> EngineResult<Option<UiInputCaptureState>> {
        if primary_state.visible || primary_was_open {
            // Publish both visible and hidden states. engine.ui owns retained node
            // lifecycle; if runtime does not send the hidden node on close, the
            // provider can legally keep the previous retained menu on screen.
            ui_gateway::publish_surface_node(primary_state);
        }

        let external_refresh =
            external_capture.draw_refresh_requested || external_capture.requests_capture();

        if !primary_state.visible && !primary_was_open && !external_refresh {
            return Ok(None);
        }

        let needs_clear_packet = (!primary_state.visible && primary_was_open)
            || (external_capture.draw_refresh_requested && !external_capture.requests_capture());

        let mut provider_capture = None;
        match ui_gateway::request_frame_output(
            self.frame.frame_index,
            scope.dt,
            [scope.w, scope.h],
            1.0,
        ) {
            Ok(Some(output)) => {
                provider_capture = Some(output.input_capture);
                *ui = Some(output.draw_list);
            }
            Ok(None) => {
                if needs_clear_packet {
                    *ui = Some(clear_ui_draw_list([scope.w, scope.h]));
                }
            }
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "modal ui: same-frame draw-list refresh failed: {e}"
                );
                if needs_clear_packet {
                    *ui = Some(clear_ui_draw_list([scope.w, scope.h]));
                }
            }
        }

        Ok(provider_capture)
    }

    fn read_viewport_frame_input<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
    ) -> ViewportFrameInput {
        let surface_input = ctx
            .resources()
            .get::<newengine_ui_api::UiInputFrame>()
            .cloned();
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
        let play_mode =
            editor_viewport_play_mode(ctx).unwrap_or_else(|| self.bridges.scene.play_mode());
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
        // The scene launch gate is an early-return path: it does not go through
        // the normal frame-envelope UI composite pass. Reuse the UI-only submit
        // path so retained loading visuals with UiPaintCommand::Image are staged
        // with a valid viewport/scissor and can be composited before end_frame().
        self.render_ui_only_frame(ctx, r, ui, scope)?;
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

fn merge_ui_input_capture(
    mut out: UiInputCaptureState,
    incoming: UiInputCaptureState,
) -> UiInputCaptureState {
    out.sampling_alive = true;
    out.camera_navigation_gated |= incoming.camera_navigation_gated;
    out.gameplay_movement_gated |= incoming.gameplay_movement_gated;
    out.modal |= incoming.modal;
    out.draw_refresh_requested |= incoming.draw_refresh_requested;
    for surface in incoming.surfaces {
        if !out.surfaces.iter().any(|it| it == &surface) {
            out.surfaces.push(surface);
        }
    }
    for contributor in incoming.contributors {
        if !out.contributors.iter().any(|it| it == &contributor) {
            out.contributors.push(contributor);
        }
    }
    let incoming_reason = incoming.reason.trim();
    if !incoming_reason.is_empty() && incoming_reason != "none" {
        if out.reason.trim().is_empty() || out.reason == "none" {
            out.reason = incoming.reason;
        } else if out.reason != incoming.reason {
            out.reason = format!("{} + {}", out.reason, incoming.reason);
        }
    }
    out
}

fn clear_ui_draw_list(surface_size_px: [u32; 2]) -> UiDrawList {
    let mut draw_list = UiDrawList::new();
    draw_list.screen_size_px = surface_size_px;
    draw_list.pixels_per_point = 1.0;
    draw_list
}

fn is_game_screen_profile<E: Send + 'static>(ctx: &ModuleCtx<'_, E>) -> bool {
    ctx.resources()
        .get::<UiScreenProfileState>()
        .map(|state| state.descriptor.profile == UiScreenProfile::Game)
        .unwrap_or(true)
}

fn editor_viewport_runtime_mode<E: Send + 'static>(
    ctx: &ModuleCtx<'_, E>,
) -> Option<UiEditorRuntimeMode> {
    let profile = ctx
        .resources()
        .get::<UiScreenProfileState>()
        .map(|state| state.descriptor.profile)
        .unwrap_or_default();
    if profile != UiScreenProfile::Editor {
        return None;
    }
    Some(
        ctx.resources()
            .get::<UiEditorRuntimeState>()
            .map(|state| state.mode)
            .unwrap_or(UiEditorRuntimeMode::Edit),
    )
}

fn editor_viewport_play_mode<E: Send + 'static>(
    ctx: &ModuleCtx<'_, E>,
) -> Option<crate::gameplay::GameRunMode> {
    let mode = editor_viewport_runtime_mode(ctx)?;
    Some(match mode {
        UiEditorRuntimeMode::Edit => crate::gameplay::GameRunMode::Staging,
        UiEditorRuntimeMode::Simulate => crate::gameplay::GameRunMode::Simulate,
        UiEditorRuntimeMode::Play => crate::gameplay::GameRunMode::Play,
    })
}
