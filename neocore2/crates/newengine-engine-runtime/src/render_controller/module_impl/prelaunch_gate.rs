use newengine_core::host_events::CursorState;
use newengine_core::render::{RenderApi, RenderWorkBudget, SceneLaunchStatus, UploadPumpDesc};
use newengine_core::{EngineResult, ModuleCtx};

use super::launch_loading::scene_launch_loading_status;
use super::readiness;
use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(super) fn handle_native_prelaunch_gate<E: Send + 'static>(
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

        self.bridges.scene.apply_commands();
        {
            let scene_lock = self.bridges.scene.scene();
            let mut scene = scene_lock.write();
            let has_pending_gate = scene
                .world()
                .resource::<crate::gameplay::GameReadyWorldLaunchGate>()
                .map(|gate| !gate.is_play_activated())
                .unwrap_or(false);

            if has_pending_gate {
                let requested_play_mode = self.bridges.scene.play_mode();
                let world_playable = scene.run_frame(next_frame, |world| {
                    readiness::update_game_ready_launch_gate(
                        self,
                        r,
                        world,
                        requested_play_mode,
                        next_frame,
                    )
                });

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
                        gate.mark_play_activated();
                        prelaunch_released = true;
                    }
                    prelaunch_gate = Some(gate.clone());
                }
            }
        }

        let Some(gate) = prelaunch_gate else {
            return Ok(None);
        };

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
                    "render controller: native loading frame={} reason='{}'",
                    self.frame.frame_index,
                    gate.reason
                );
            }
            scene_launch_loading_status(&gate)
        };

        Ok(Some(status))
    }
}
