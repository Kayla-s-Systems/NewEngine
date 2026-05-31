use newengine_core::host_events::CursorState;
use newengine_ui_api::{UiEditorRuntimeMode, UiEditorRuntimeState, UiScreenProfile, UiScreenProfileState};
use newengine_core::render::{RenderApi, RenderWorkBudget, SceneLaunchStatus, UploadPumpDesc};
use newengine_core::{EngineResult, ModuleCtx};

use super::launch_loading::scene_launch_loading_status;
use super::readiness;
use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(super) fn handle_prelaunch_gate<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        r: &mut dyn RenderApi,
        backend_work_budget: Option<RenderWorkBudget>,
        material_upload_jobs: u32,
        trace_frame: bool,
    ) -> EngineResult<Option<SceneLaunchStatus>> {
        let next_frame = self.frame.frame_index.saturating_add(1).max(1);
        let mut prelaunch_gate = None;
        let mut prelaunch_released = false;
        let mut editor_preview_ready = false;
        let editor_runtime_mode = editor_runtime_mode(ctx);
        let editor_preview_blocks_auto_play = editor_runtime_mode == Some(UiEditorRuntimeMode::Edit);

        self.bridges.scene.apply_commands();
        {
            let scene_lock = self.bridges.scene.scene();
            let mut scene = scene_lock.write();
            let has_pending_gate = scene
                .world()
                .resource::<crate::gameplay::GameReadyWorldLaunchGate>()
                .map(|gate| gate.needs_prelaunch_gate())
                .unwrap_or(false);

            if has_pending_gate {
                let requested_play_mode = self.bridges.scene.play_mode();
                let runtime_profile = self.runtime_profile().clone();
                let job_system = ctx.job_system().cloned();
                let world_playable = scene.run_frame(next_frame, |world| {
                    if runtime_profile.use_runtime_terrain_streaming() {
                        let mats_lock = self.bridges.scene.materials();
                        let mats = mats_lock.read();
                        crate::scene_bridge::tick_game_ready_streaming_terrain(
                            world,
                            &mats,
                            job_system.as_ref(),
                        );
                    }
                    readiness::update_game_ready_launch_gate(
                        self,
                        r,
                        world,
                        requested_play_mode,
                        next_frame,
                    )
                });

                // Launch gate must pump world/terrain residency too. Otherwise
                // `Loading World` can reach texture 100% while the world still has
                // CPU-prepared terrain chunks waiting for GPU packets, and redraws
                // may only advance again after unrelated UI input.
                if let Err(e) = self.pump_scene_gpu_residency(r, &scene) {
                    log::warn!("render prelaunch: terrain GPU residency pump failed: {}", e);
                }

                self.pump_material_texture_requests(
                    r,
                    ctx.job_system(),
                    super::super::render_quality::MATERIAL_TEXTURE_IMPORT_START_BURST.min(material_upload_jobs.max(1)),
                    material_upload_jobs,
                );
                let upload_desc = backend_work_budget
                    .map(|budget| UploadPumpDesc::loading_screen_warmup().with_budget(budget))
                    .unwrap_or_else(UploadPumpDesc::loading_screen_warmup);
                let _ = r.pump_uploads(upload_desc);

                if world_playable {
                    if let Err(e) = self.prewarm_scene_gpu_resources(r, &scene) {
                        log::warn!(
                            "render prewarm: failed during launch gate handoff err='{}'",
                            e
                        );
                    }
                }

                if let Some(gate) = scene
                    .world_mut()
                    .resource_mut::<crate::gameplay::GameReadyWorldLaunchGate>()
                {
                    if world_playable && !gate.is_play_activated() {
                        if editor_preview_blocks_auto_play {
                            gate.mark_editor_preview_ready(
                                next_frame,
                                "editor preview ready; simulation stopped until Simulate or Play",
                            );
                            editor_preview_ready = true;
                        } else {
                            gate.mark_play_activated();
                            prelaunch_released = true;
                        }
                    }
                    prelaunch_gate = Some(gate.clone());
                }
            }
        }

        let Some(gate) = prelaunch_gate else {
            return Ok(None);
        };

        if editor_preview_ready {
            log::info!(
                "editor launch gate: preview ready frame={} mode='edit' reason='{}'; simulation remains stopped",
                next_frame,
                gate.reason
            );
            return Ok(None);
        }

        self.frame.frame_index = next_frame;
        self.sync_cursor_state(ctx, CursorState::released());
        let _ = r.discard_recorded_commands();

        let status = if prelaunch_released {
            self.bridges.scene.activate_profile_play_now();
            self.diagnostics.overlay_metrics.reset_interactive_timing();
            log::info!(
                "render controller: scene launch gate released; deferring first world present to next frame"
            );
            SceneLaunchStatus::loading(
                "NEWENGINE // LOADING WORLD",
                "Playable world is ready.",
                "Preparing the first stable gameplay frame.",
                0.995,
            )
        } else {
            if trace_frame {
                log::debug!(
                    "render controller: loading gate frame={} reason='{}'",
                    self.frame.frame_index,
                    gate.reason
                );
            }
            scene_launch_loading_status(&gate)
        };

        Ok(Some(status))
    }
}


fn editor_runtime_mode<E: Send + 'static>(ctx: &ModuleCtx<'_, E>) -> Option<UiEditorRuntimeMode> {
    let screen_profile = ctx
        .resources()
        .get::<UiScreenProfileState>()
        .map(|state| state.descriptor.profile)
        .unwrap_or_default();
    if screen_profile != UiScreenProfile::Editor {
        return None;
    }
    Some(
        ctx.resources()
            .get::<UiEditorRuntimeState>()
            .map(|state| state.mode)
            .unwrap_or(UiEditorRuntimeMode::Edit),
    )
}
