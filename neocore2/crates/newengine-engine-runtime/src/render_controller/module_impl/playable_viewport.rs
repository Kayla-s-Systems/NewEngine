use crate::input_systems::InputCaptureState;
use crate::ui_gateway;
use newengine_core::host_events::CursorState;
use newengine_core::physics::PhysicsApiRef;
use newengine_core::render::{Extent2D, RenderApi};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_runtime_session_api::{RuntimeSessionMode, RuntimeSessionState};
use newengine_runtime_session_runtime::{
    begin_runtime_session_frame, record_runtime_session_ticks,
};
use newengine_ui_api::{
    UiDrawCmd, UiDrawList, UiEditorRuntimeMode, UiInputCaptureState, UiRect,
    UiRuntimeDebugOverlayTelemetry, UiScreenProfile, UiScreenProfileState, UiSurfaceNode, UiTexId,
    UiVertex, UiViewportSlot,
};

use super::super::controller::RuntimeRenderController;
use super::frame_types::{PlayableFrameOutcome, RenderFrameScope, ViewportFrameInput};
use super::input::ViewportInputSnap;

mod early_exit;
mod editor;
mod input;
mod ui;

use ui::*;

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
        let (editor_staging_preview, in_game_editor) =
            self.prepare_editor_interaction(ctx, &frame_input, scope);

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
        let gameplay_capture = {
            let scene_lock = self.bridges.scene.scene();
            let scene = scene_lock.read();
            self.frame
                .gameplay_ui
                .aggregate_input_capture(scene.world())
        };
        let session_frame =
            begin_runtime_session_frame(ctx.resources_mut(), self.frame.frame_index);
        let session_ejected = session_frame.active
            && session_frame.mode == Some(RuntimeSessionMode::Play)
            && !session_frame.possessed;
        // Preserve the UI capture contract channel-by-channel. A movement-only widget
        // must not kill camera look, and a pointer/camera capture must not implicitly
        // disable locomotion. Only true modal/editor/session ownership gates both.
        let force_full_runtime_gate =
            in_game_editor || editor_staging_preview || session_frame.paused || session_ejected;
        let host_capture = InputCaptureState {
            sampling_alive: true,
            camera_navigation_gated: force_full_runtime_gate
                || published_capture.modal
                || published_capture.camera_navigation_gated,
            gameplay_movement_gated: force_full_runtime_gate
                || published_capture.modal
                || published_capture.gameplay_movement_gated,
            reason: if force_full_runtime_gate {
                "engine.host.runtime-ownership"
            } else if published_capture.modal {
                "engine.ui.modal"
            } else if published_capture.camera_navigation_gated
                || published_capture.gameplay_movement_gated
            {
                "engine.ui.selective-capture"
            } else {
                "clear"
            },
        };
        // Selective input capture is not a simulation pause. Pausing on any capture made
        // transient hover/focus states freeze the world and made the camera appear locked.
        let pause_world = published_capture.modal
            || in_game_editor
            || editor_staging_preview
            || gameplay_capture.pause_simulation
            || (session_frame.paused && !session_frame.step_this_frame);
        let session_fixed_step_count = if session_frame.step_this_frame {
            1
        } else {
            scope.fixed_step_count
        };
        if primary_was_open && !primary_ui.blocks_gameplay {
            self.restore_playable_view_after_ui_close();
        }
        if primary_ui.exit_requested {
            newengine_ulog_api::ulog::info!(
                "UI surface: exit requested through declarative menu action"
            );
            ctx.request_exit();
        }
        if self.frame.last_play_mode.is_runtime() && !frame_input.play_mode.is_runtime() {
            let scene_lock = self.bridges.scene.scene();
            let mut scene = scene_lock.write();
            self.frame
                .gameplay_ui
                .reset_transient_state(scene.world_mut());
        }
        {
            let mut carrier = frame_input.input.action_carrier();
            self.frame.input_systems.publish_input_capture_state(
                self.frame.frame_index,
                host_capture,
                &mut carrier,
            );
            carrier.apply_gameplay_input_capture(gameplay_capture);
        }

        // Editor/Edit is a staging-world preview, not a UI-only shell. The authored
        // scene is rendered through the normal world/material/camera path while
        // `pause_world` prevents physics/gameplay fixed steps. Simulate/Play only
        // change runtime ownership; they are not required for the editor to see its map.

        if self.app_policy.idle_preview_uses_ui_only(
            self.bridges.viewport.external_extent_owned(),
            self.bridges.viewport.external_redraw_requested(),
        ) {
            // Standalone asset tools must not initialize gameplay pipelines or
            // tick an empty world while no 3D preview is requested. A preview API
            // explicitly owns the offscreen extent for a real 3D preview. Once
            // its warmup/redraw budget is exhausted, UI keeps sampling the cached
            // target while the expensive world/render path sleeps.
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
        let selected = self.bridges.scene.selection();
        if editor_staging_preview {
            let selection_radius = super::scene::selection_bounds_world(scene.world(), selected)
                .map(|bounds| bounds.radius)
                .unwrap_or(0.5);
            self.editor_viewport.sync_gizmo_geometry(
                &self.bridges.scene,
                &mut scene,
                selected,
                selection_radius,
            );
            let last_camera = self.frame.last_camera_snapshot.as_ref();
            self.editor_viewport.process_history_actions(
                scene.world_mut(),
                ctx.resources()
                    .get::<newengine_ui_api::UiEventDispatchFrame>(),
            );
            self.editor_viewport.process_transform_input(
                scene.world_mut(),
                selected,
                frame_input.surface_input.as_ref(),
                last_camera,
                [scope.vp_w, scope.vp_h],
            );
        } else {
            self.editor_viewport
                .clear_runtime_geometry(scene.world_mut());
        }
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
            scope.fixed_alpha,
            session_fixed_step_count,
            scope.fixed_tick,
            pause_world,
            scope.aspect(),
            scope.vp_w,
            scope.vp_h,
        );
        if session_frame.active && !pause_world {
            record_runtime_session_ticks(ctx.resources_mut(), session_fixed_step_count);
        }
        self.frame
            .gameplay_ui
            .publish_frame(scene.world_mut(), self.frame.frame_index);
        let gameplay_capture_after_tick = self
            .frame
            .gameplay_ui
            .aggregate_input_capture(scene.world());

        if !world_frame.view_frame.world_playable {
            let ui_telemetry =
                self.end_frame_for_unplayable_world(ctx, r, &scene, frame_input.ui, scope)?;
            return Ok(PlayableFrameOutcome::EndedEarly {
                ui_telemetry: Some(ui_telemetry),
            });
        }

        if host_capture.camera_navigation_gated || gameplay_capture_after_tick.release_cursor {
            // UI capture must visibly release the OS cursor even if runtime-side
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
        )?;
        drop(scene);
        if self.editor_viewport.take_inspector_dirty() {
            self.bridges.scene.refresh_editor_inspector();
        }
        if matches!(outcome, PlayableFrameOutcome::Continue { .. })
            && self.external_preview_target_active()
        {
            self.bridges.viewport.mark_external_redraw_presented();
        }
        if let Some(picked) = self.frame.pending_pick_selection.take() {
            let additive = core::mem::take(&mut self.frame.pending_pick_additive);
            if additive {
                if let Some(entity) = picked {
                    self.bridges.scene.toggle_selection(entity);
                }
            } else {
                self.bridges.scene.set_selection(picked);
            }
        }
        if editor_staging_preview {
            let (scene_snapshot, inspector_snapshot) = self
                .bridges
                .scene
                .editor_scene_snapshots(self.frame.frame_index);
            ctx.resources_mut().insert(scene_snapshot);
            ctx.resources_mut().insert(inspector_snapshot);
        }
        Ok(outcome)
    }
}
