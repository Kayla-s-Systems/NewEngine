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
                .resource::<crate::gameplay::GameReadyWorldLaunchGate>()
                .map(|gate| gate.needs_prelaunch_gate())
                .unwrap_or(false);

            if has_pending_gate {
                let requested_play_mode = self.bridges.scene.play_mode();
                let prims_lock = self.bridges.scene.primitives();
                let mut prims = prims_lock.write();
                let mats_lock = self.bridges.scene.materials();
                let mats = mats_lock.read();
                let world_playable = scene.run_frame(next_frame, |world| {
                    // Static authored world assembly is incremental and must progress
                    // inside the prelaunch path. The normal world tick is intentionally
                    // bypassed while the gate is active, so admitting it only there would
                    // starve the queue until the soft timeout.
                    crate::scene_bridge::tick_game_ready_static_world_prefabs(
                        world,
                        &mut prims,
                        &mats,
                        ctx.thread_pool(),
                    );
                    // Prelaunch may pump GPU residency for the already materialized launch
                    // ring, but must not admit additional CPU terrain chunks. Otherwise the
                    // launch gate target keeps growing before public Play, and the window
                    // remains blocked in loading while streaming expands toward the full
                    // runtime radius. Full streaming admission resumes from world_tick after
                    // the launch gate releases.
                    readiness::update_game_ready_launch_gate(
                        self,
                        r,
                        world,
                        requested_play_mode,
                        next_frame,
                    )
                });
                // The residency pump below reads the primitive registry. Release
                // the static-world admission write guard first; otherwise the
                // prelaunch thread deadlocks itself on the same RwLock.
                drop(prims);
                drop(mats);

                // Launch gate must pump world/terrain residency too. Otherwise
                // `Loading World` can reach texture 100% while the world still has
                // CPU-prepared terrain chunks waiting for GPU packets, and redraws
                // may only advance again after unrelated UI input.
                if let Err(e) = self.pump_scene_gpu_residency(r, &scene) {
                    newengine_ulog_api::ulog::warn!(
                        "render prelaunch: terrain GPU residency pump failed: {}",
                        e
                    );
                }

                self.pump_material_texture_requests(
                    r,
                    ctx.thread_pool(),
                    super::super::render_quality::MATERIAL_TEXTURE_IMPORT_START_BURST
                        .min(material_upload_jobs.max(1)),
                    material_upload_jobs,
                );
                let upload_desc = backend_work_budget
                    .map(|budget| UploadPumpDesc::loading_screen_warmup().with_budget(budget))
                    .unwrap_or_else(UploadPumpDesc::loading_screen_warmup);
                let _ = r.pump_uploads(upload_desc);

                if world_playable {
                    if let Err(e) = self.prewarm_scene_gpu_resources(r, &scene) {
                        newengine_ulog_api::ulog::warn!(
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
