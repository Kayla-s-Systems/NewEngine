use newengine_core::host_events::CursorState;
use newengine_core::render::{
    BeginFrameDesc, Extent2D, RectI32, RenderApi, RenderWorkBudget, SceneLaunchStatus,
    UploadPumpDesc, Viewport,
};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_ui_api::{
    UiDrawList, UiEditorRuntimeMode, UiEditorRuntimeState, UiScreenProfile, UiScreenProfileState,
};

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
        let mut editor_preview_ready = false;
        let editor_runtime_mode = editor_runtime_mode(ctx);
        let editor_preview_blocks_auto_play =
            editor_runtime_mode == Some(UiEditorRuntimeMode::Edit);

        self.bridges.scene.apply_commands();
        {
            let scene_lock = self.bridges.scene.scene();
            let mut scene = scene_lock.write();
            let has_pending_gate = scene
                .world()
                .resource::<crate::gameplay::WorldActivationState>()
                .map(|gate| gate.needs_prelaunch_gate())
                .unwrap_or(false);

            if has_pending_gate {
                // Prelaunch is a real presented frame. Advance the controller index
                // before scheduling work so task ids, retry ages and residency
                // intervals all refer to the frame currently being prepared.
                self.frame.frame_index = next_frame;
                let requested_play_mode = self.bridges.scene.play_mode();
                let prims_lock = self.bridges.scene.primitives();
                let mut prims = prims_lock.write();
                let mats_lock = self.bridges.scene.materials();
                let mats = mats_lock.read();
                let material_plan = scene.run_frame(next_frame, |world| {
                    // Static authored world assembly is incremental and must progress
                    // inside the prelaunch path. The normal world tick is intentionally
                    // bypassed while the gate is active, so admitting it only there would
                    // starve the queue until the soft timeout.
                    self.frame.world_runtime.tick_prelaunch(
                        world,
                        &mut prims,
                        &mats,
                        ctx.thread_pool(),
                        next_frame,
                    );

                    // Queue only launch-critical textures, with alpha-tested base
                    // textures first. Optional environment maps remain post-launch
                    // streaming work and cannot consume the limited decode slots.
                    readiness::prepare_scene_launch_resources(self, world, &*mats)
                });
                // The residency pump below reads the primitive/material registries.
                // Release static-world admission guards first to avoid self-deadlock.
                drop(prims);
                drop(mats);

                // Admit bounded terrain/primitive packets before evaluating readiness.
                // The previous order checked readiness first, so work completed by this
                // frame's pump could not release Play until the following frame.
                if let Err(e) = self.pump_scene_gpu_residency(r, &scene) {
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

                // Pipeline construction belongs under the loading projection, not
                // in the first public gameplay frame. Mesh uploads remain bounded by
                // pump_scene_gpu_residency and are never swept synchronously here.
                if let Err(e) = self.prewarm_scene_pipeline(r) {
                    if next_frame <= 4 || next_frame.is_multiple_of(120) {
                        newengine_ulog_api::ulog::warn!(
                            "render prewarm: primary scene pipeline not ready frame={} err='{}'",
                            next_frame,
                            e
                        );
                    }
                }

                // Evaluate against the resource state produced by this same frame's
                // CPU decode, GPU upload and pipeline warmup work.
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
                    .resource_mut::<crate::gameplay::WorldActivationState>()
                {
                    if world_playable && !gate.is_active() {
                        if editor_preview_blocks_auto_play {
                            gate.mark_preview_ready(
                                next_frame,
                                "editor preview ready; simulation stopped until Simulate or Play",
                            );
                            editor_preview_ready = true;
                        } else {
                            gate.mark_active();
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
            newengine_ulog_api::ulog::info!(
                "editor launch gate: preview ready frame={} mode='edit' reason='{}'; simulation remains stopped",
                next_frame,
                gate.reason
            );
            return Ok(None);
        }

        self.frame.frame_index = next_frame;
        self.sync_cursor_state(ctx, CursorState::released());
        let _ = r.discard_recorded_commands();

        // This prelaunch gate returns before `render_runtime_module` consumes the
        // provider-owned UiDrawList and before the normal frame envelope can run.
        // Present a minimal UI-only frame here so `engine.ui.loading` image paints
        // are actually composited while scene texture residency is blocking Play.
        if let Some(mut ui) = ctx.resources().get::<UiDrawList>().cloned() {
            let paint_images = ui
                .paint
                .commands
                .iter()
                .filter(|cmd| matches!(cmd, newengine_ui_api::UiPaintCommand::Image(_)))
                .count();
            let original_set = ui.texture_delta.set.len();
            let original_patches = ui.texture_delta.patches.len();
            let original_bytes = ui
                .texture_delta
                .set
                .values()
                .map(|texture| texture.rgba8.len())
                .sum::<usize>()
                + ui.texture_delta
                    .patches
                    .iter()
                    .map(|patch| patch.rgba8.len())
                    .sum::<usize>();
            self.filter_redundant_prelaunch_texture_delta(&mut ui);
            let submitted_bytes = ui
                .texture_delta
                .set
                .values()
                .map(|texture| texture.rgba8.len())
                .sum::<usize>()
                + ui.texture_delta
                    .patches
                    .iter()
                    .map(|patch| patch.rgba8.len())
                    .sum::<usize>();
            newengine_ulog_api::ulog::warn!(
                "render prelaunch loading ui: present frame={} window={}x{} mesh_cmds={} paint_cmds={} paint_images={} tex_set={}/{} patches={}/{} tex_bytes={}/{} reason='{}'",
                next_frame,
                window_w,
                window_h,
                ui.mesh.cmds.len(),
                ui.paint.commands.len(),
                paint_images,
                ui.texture_delta.set.len(),
                original_set,
                ui.texture_delta.patches.len(),
                original_patches,
                submitted_bytes,
                original_bytes,
                gate.reason
            );
            present_prelaunch_loading_ui_frame(
                r,
                ui,
                self.viewport.clear_color,
                next_frame,
                window_w,
                window_h,
            )?;
        } else {
            newengine_ulog_api::ulog::warn!(
                "render prelaunch loading ui: missing UiDrawList in resources frame={} reason='{}'",
                next_frame,
                gate.reason
            );
        }

        let status = if prelaunch_released {
            self.bridges.scene.activate_profile_play_now();
            self.diagnostics.overlay_metrics.reset_interactive_timing();
            newengine_ulog_api::ulog::info!(
                "render controller: scene launch gate released; loading overlay deactivated; deferring first world present to next frame"
            );
            // The launch gate is released at this point. Returning another active
            // loading status keeps engine.ui.loading alive for the next runtime
            // frame and can leave a small loading menu over the first gameplay
            // presents. The next frame is intentionally deferred, but the loading
            // overlay lifecycle must already be inactive.
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

    fn filter_redundant_prelaunch_texture_delta(&mut self, ui: &mut UiDrawList) {
        for id in &ui.texture_delta.free {
            self.ui.prelaunch_texture_fingerprints.remove(&id.0);
            self.ui
                .prelaunch_patch_fingerprints
                .retain(|key, _| key.0 != id.0);
        }

        ui.texture_delta.set.retain(|id, texture| {
            let fingerprint = ui_payload_fingerprint(texture.size, &texture.rgba8);
            !matches!(
                self.ui
                    .prelaunch_texture_fingerprints
                    .insert(id.0, fingerprint),
                Some(previous) if previous == fingerprint
            )
        });

        ui.texture_delta.patches.retain(|patch| {
            let key = (
                patch.id.0,
                patch.origin[0],
                patch.origin[1],
                patch.size[0],
                patch.size[1],
            );
            let fingerprint = ui_payload_fingerprint(patch.size, &patch.rgba8);
            !matches!(
                self.ui
                    .prelaunch_patch_fingerprints
                    .insert(key, fingerprint),
                Some(previous) if previous == fingerprint
            )
        });
    }
}

#[inline]
fn ui_payload_fingerprint(size: [u32; 2], rgba8: &[u8]) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size[0].to_le_bytes());
    hasher.update(&size[1].to_le_bytes());
    hasher.update(rgba8);
    let hash = hasher.finalize();
    u64::from_le_bytes(hash.as_bytes()[0..8].try_into().expect("blake3 prefix"))
}

fn present_prelaunch_loading_ui_frame(
    r: &mut dyn RenderApi,
    ui: UiDrawList,
    clear_color: [f32; 4],
    frame_index: u64,
    window_w: u32,
    window_h: u32,
) -> EngineResult<()> {
    if window_w == 0 || window_h == 0 {
        return Ok(());
    }

    r.begin_frame(BeginFrameDesc::new(clear_color).with_frame_index(frame_index))?;
    let extent = Extent2D::new(window_w, window_h);
    r.set_viewport(Viewport::full(extent))?;
    r.set_scissor(RectI32::new(0, 0, window_w as i32, window_h as i32))?;
    r.set_ui_draw_list(ui);
    r.end_frame()
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
    fn prelaunch_decode_jobs_respect_runtime_ceiling() {
        assert_eq!(
            prelaunch_material_decode_jobs(u32::MAX),
            super::super::super::render_quality::MATERIAL_TEXTURE_MAX_ASYNC_DECODE_JOBS as u32
        );
    }
}
