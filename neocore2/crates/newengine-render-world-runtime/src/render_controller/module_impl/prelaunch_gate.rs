use newengine_core::host_events::CursorState;
use newengine_core::render::{RenderApi, RenderWorkBudget, SceneLaunchStatus, UploadPumpDesc};
use newengine_core::{EngineResult, ModuleCtx};

use super::super::controller::RuntimeRenderController;
use super::launch_loading::scene_launch_loading_status;
use super::readiness;

impl RuntimeRenderController {
    pub(super) fn handle_prelaunch_gate<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        r: &mut dyn RenderApi,
        backend_work_budget: Option<RenderWorkBudget>,
        material_upload_jobs: u32,
        trace_frame: bool,
        window_w: u32,
        window_h: u32,
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
                .resource::<newengine_gameplay_world_runtime::gameplay::WorldActivationState>()
                .map(|gate| gate.needs_prelaunch_gate())
                .unwrap_or(false);

            if has_pending_gate {
                let physics_api = newengine_core::physics::require_physics_api(ctx)
                    .ok()
                    .cloned();
                self.frame.frame_index = next_frame;
                let requested_play_mode = self.bridges.scene.play_mode();
                let prims_lock = self.bridges.scene.primitives();
                let mut prims = prims_lock.write();
                let mats_lock = self.bridges.scene.materials();
                let mats = mats_lock.read();
                let material_plan = scene.run_frame(next_frame, |world| {
                    if let Some(physics_api) = physics_api.as_ref() {
                        newengine_gameplay_world_runtime::gameplay::prewarm_service_physics_backend(
                            world,
                            physics_api,
                        );
                    }
                    self.frame.world_runtime.tick_prelaunch(
                        world,
                        &mut prims,
                        &mats,
                        ctx.thread_pool(),
                        next_frame,
                    );
                    if let Some(physics_api) = physics_api.as_ref() {
                        newengine_gameplay_world_runtime::gameplay::sync_prelaunch_service_physics(
                            world,
                            physics_api,
                        );
                    }
                    readiness::prepare_scene_launch_resources(self, world, &*mats)
                });
                drop(prims);
                drop(mats);

                if let Err(e) = self.pump_scene_gpu_residency(r, &scene, ctx.thread_pool()) {
                    newengine_ulog_api::ulog::warn!(
                        "render prelaunch: scene GPU residency pump failed: {}",
                        e
                    );
                }

                let decode_jobs = prelaunch_material_decode_jobs(material_upload_jobs);
                self.pump_material_texture_requests(
                    r,
                    ctx.thread_pool(),
                    super::super::render_quality::MATERIAL_TEXTURE_IMPORT_START_BURST
                        .min(decode_jobs),
                    decode_jobs,
                );

                let upload_budget = loading_screen_work_budget(backend_work_budget);
                let upload_desc =
                    UploadPumpDesc::loading_screen_warmup().with_budget(upload_budget);
                match r.pump_uploads(upload_desc) {
                    Ok(report) => {
                        if report.failed_jobs > 0 {
                            newengine_ulog_api::ulog::warn!(
                                "render prelaunch: upload pump completed with failures frame={} processed_jobs={} processed_bytes={} remaining_jobs={} remaining_bytes={} failed_jobs={}",
                                next_frame,
                                report.processed_jobs,
                                report.processed_bytes,
                                report.remaining_jobs,
                                report.remaining_bytes,
                                report.failed_jobs,
                            );
                        } else if trace_frame
                            && (report.processed_jobs > 0 || report.remaining_jobs > 0)
                        {
                            newengine_ulog_api::ulog::debug!(
                                "render prelaunch: upload pump frame={} processed_jobs={} processed_bytes={} remaining_jobs={} remaining_bytes={} budget_blocked={}",
                                next_frame,
                                report.processed_jobs,
                                report.processed_bytes,
                                report.remaining_jobs,
                                report.remaining_bytes,
                                report.blocked_by_budget,
                            );
                        }
                    }
                    Err(e) => {
                        newengine_ulog_api::ulog::warn!(
                            "render prelaunch: upload pump failed frame={} err='{}'",
                            next_frame,
                            e
                        );
                    }
                }

                if let Err(e) = self.prewarm_scene_pipeline(r, window_w, window_h) {
                    if next_frame <= 4 || next_frame.is_multiple_of(120) {
                        newengine_ulog_api::ulog::warn!(
                            "render prewarm: primary scene pipeline not ready frame={} err='{}'",
                            next_frame,
                            e
                        );
                    }
                }

                let world_playable = scene.run_frame(next_frame, |world| {
                    readiness::update_world_activation_gate_with_material_plan(
                        self,
                        r,
                        world,
                        requested_play_mode,
                        &material_plan,
                        next_frame,
                    )
                });

                if let Some(gate) = scene
                    .world_mut()
                    .resource_mut::<newengine_gameplay_world_runtime::gameplay::WorldActivationState>()
                {
                    if world_playable && !gate.is_active() {
                        gate.mark_active();
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
        // Prelaunch owns GPU/resource warmup only. Window presentation remains exclusively
        // owned by the platform-native loading compositor until SceneLaunchStatus releases.
        // Submitting even an empty Vulkan frame here clears/presents the swapchain over the
        // native loader and makes the loading stage visually disappear.
        let _ = r.discard_recorded_commands();

        let status = if prelaunch_released {
            self.bridges.scene.activate_profile_play_now();
            self.diagnostics.overlay_metrics.reset_interactive_timing();
            newengine_ulog_api::ulog::info!(
                "render controller: scene launch gate released; loading overlay deactivated; deferring first world present to next frame"
            );
            SceneLaunchStatus::inactive()
        } else {
            if trace_frame {
                newengine_ulog_api::ulog::debug!(
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

fn prelaunch_material_decode_jobs(configured_jobs: u32) -> u32 {
    let ceiling = super::super::render_quality::MATERIAL_TEXTURE_MAX_ASYNC_DECODE_JOBS as u32;
    configured_jobs
        .max(1)
        .saturating_mul(2)
        .max(super::super::render_quality::MATERIAL_TEXTURE_IMPORT_START_BURST)
        .min(ceiling.max(1))
}

fn loading_screen_work_budget(base: Option<RenderWorkBudget>) -> RenderWorkBudget {
    let mut budget = base.unwrap_or_default();
    budget.max_upload_bytes_per_frame = budget
        .max_upload_bytes_per_frame
        .saturating_mul(2)
        .clamp(8 * 1024 * 1024, 64 * 1024 * 1024);
    budget.max_upload_jobs_per_frame = budget
        .max_upload_jobs_per_frame
        .max(1)
        .saturating_mul(2)
        .clamp(2, 16);
    budget.max_pipeline_builds_per_frame = budget
        .max_pipeline_builds_per_frame
        .max(1)
        .saturating_mul(2)
        .clamp(1, 4);
    budget.max_blocking_ms_per_frame = if budget.max_blocking_ms_per_frame.is_finite() {
        budget.max_blocking_ms_per_frame.clamp(6.0, 12.0)
    } else {
        6.0
    };
    budget
}

#[cfg(test)]
mod loading_budget_tests {
    use super::*;

    #[test]
    fn loading_budget_is_more_aggressive_but_bounded() {
        let base = RenderWorkBudget::default();
        let loading = loading_screen_work_budget(Some(base));
        assert!(loading.max_upload_bytes_per_frame > base.max_upload_bytes_per_frame);
        assert!(loading.max_upload_jobs_per_frame > base.max_upload_jobs_per_frame);
        assert!((6.0..=12.0).contains(&loading.max_blocking_ms_per_frame));
    }

    #[test]
    fn prelaunch_gate_never_submits_or_begins_a_window_frame() {
        let source = include_str!("prelaunch_gate.rs");
        let submit_frame = ["r.sub", "mit_frame("].concat();
        let begin_frame = ["r.beg", "in_frame("].concat();
        let end_frame = ["r.end", "_frame("].concat();
        assert!(!source.contains(&submit_frame));
        assert!(!source.contains(&begin_frame));
        assert!(!source.contains(&end_frame));
    }

    #[test]
    fn prelaunch_decode_jobs_respect_runtime_ceiling() {
        assert_eq!(
            prelaunch_material_decode_jobs(u32::MAX),
            super::super::super::render_quality::MATERIAL_TEXTURE_MAX_ASYNC_DECODE_JOBS as u32
        );
    }
}
