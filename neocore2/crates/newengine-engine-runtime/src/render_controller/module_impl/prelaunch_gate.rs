use newengine_core::host_events::CursorState;
use newengine_core::render::{
    BeginFrameDesc, Extent2D, RenderApi, RenderWorkBudget, SceneLaunchStatus, UploadPumpDesc,
};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_math::collections::FxHashMap;
use newengine_ui_api::{UiDrawList, UiLayerDrawPacketSet};

use super::super::controller::RuntimeRenderController;
use super::frame_envelope_builder::build_ui_layer_frame_envelope;
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
                .resource::<crate::gameplay::WorldActivationState>()
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
                        crate::gameplay::prewarm_service_physics_backend(world, physics_api);
                    }
                    self.frame.world_runtime.tick_prelaunch(
                        world,
                        &mut prims,
                        &mats,
                        ctx.thread_pool(),
                        next_frame,
                    );
                    if let Some(physics_api) = physics_api.as_ref() {
                        crate::gameplay::sync_prelaunch_service_physics(world, physics_api);
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

                if let Err(e) = self.prewarm_scene_pipeline(r) {
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
                    .resource_mut::<crate::gameplay::WorldActivationState>()
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
        let _ = r.discard_recorded_commands();

        if let Some(mut ui_layers) = ctx
            .resources()
            .get::<UiLayerDrawPacketSet>()
            .cloned()
            .filter(|layers| !layers.is_empty())
        {
            let original = ui_layer_payload_stats(&ui_layers);
            self.filter_redundant_prelaunch_texture_delta(&mut ui_layers);
            let submitted = ui_layer_payload_stats(&ui_layers);
            newengine_ulog_api::ulog::debug!(
                "render prelaunch loading ui: present frame={} window={}x{} layers={} mesh_cmds={} paint_cmds={} paint_images={} tex_set={}/{} patches={}/{} tex_bytes={}/{} reason='{}'",
                next_frame,
                window_w,
                window_h,
                ui_layers.packets.len(),
                submitted.mesh_cmds,
                submitted.paint_cmds,
                submitted.paint_images,
                submitted.texture_sets,
                original.texture_sets,
                submitted.texture_patches,
                original.texture_patches,
                submitted.texture_bytes,
                original.texture_bytes,
                gate.reason
            );
            present_prelaunch_loading_ui_frame(
                r,
                ui_layers,
                self.viewport.clear_color,
                next_frame,
                window_w,
                window_h,
            )?;
        } else if next_frame <= 4 || next_frame.is_multiple_of(120) {
            newengine_ulog_api::ulog::warn!(
                "render prelaunch loading ui: missing UiLayerDrawPacketSet in resources frame={} reason='{}'",
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

    fn filter_redundant_prelaunch_texture_delta(&mut self, ui_layers: &mut UiLayerDrawPacketSet) {
        for packet in &mut ui_layers.packets {
            filter_redundant_draw_texture_delta(
                &mut self.ui.prelaunch_texture_fingerprints,
                &mut self.ui.prelaunch_patch_fingerprints,
                &mut packet.draw_list,
            );
        }
    }
}

fn filter_redundant_draw_texture_delta(
    texture_fingerprints: &mut FxHashMap<u32, u64>,
    patch_fingerprints: &mut FxHashMap<(u32, u32, u32, u32, u32), u64>,
    ui: &mut UiDrawList,
) {
    for id in &ui.texture_delta.free {
        texture_fingerprints.remove(&id.0);
        patch_fingerprints.retain(|key, _| key.0 != id.0);
    }

    ui.texture_delta.set.retain(|id, texture| {
        let fingerprint = ui_payload_fingerprint(texture.size, &texture.rgba8);
        !matches!(
            texture_fingerprints.insert(id.0, fingerprint),
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
            patch_fingerprints.insert(key, fingerprint),
            Some(previous) if previous == fingerprint
        )
    });
}

#[derive(Debug, Default, Clone, Copy)]
struct UiLayerPayloadStats {
    mesh_cmds: usize,
    paint_cmds: usize,
    paint_images: usize,
    texture_sets: usize,
    texture_patches: usize,
    texture_bytes: usize,
}

fn ui_layer_payload_stats(ui_layers: &UiLayerDrawPacketSet) -> UiLayerPayloadStats {
    let mut stats = UiLayerPayloadStats::default();
    for packet in &ui_layers.packets {
        let ui = &packet.draw_list;
        stats.mesh_cmds += ui.mesh.cmds.len();
        stats.paint_cmds += ui.paint.commands.len();
        stats.paint_images += ui
            .paint
            .commands
            .iter()
            .filter(|cmd| matches!(cmd, newengine_ui_api::UiPaintCommand::Image(_)))
            .count();
        stats.texture_sets += ui.texture_delta.set.len();
        stats.texture_patches += ui.texture_delta.patches.len();
        stats.texture_bytes += ui
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
    }
    stats
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
    ui_layers: UiLayerDrawPacketSet,
    clear_color: [f32; 4],
    frame_index: u64,
    window_w: u32,
    window_h: u32,
) -> EngineResult<()> {
    if window_w == 0 || window_h == 0 || ui_layers.is_empty() {
        return Ok(());
    }

    // `RenderApi::submit_frame` is an envelope submission contract, not a universal
    // begin-frame contract. Open the bootstrap frame explicitly so non-Vulkan backends
    // do not depend on Vulkan's defensive `if !in_frame { begin_frame(...) }` behavior.
    r.begin_frame(BeginFrameDesc::new(clear_color).with_frame_index(frame_index))?;
    let envelope = build_ui_layer_frame_envelope(
        frame_index,
        clear_color,
        Extent2D::new(window_w, window_h),
        ui_layers,
    );
    let _ = r.submit_frame(envelope)?;
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

    #[test]
    fn prelaunch_payload_stats_cover_all_layer_packets() {
        let mut layers = UiLayerDrawPacketSet::new(4);
        let mut system = UiDrawList::new();
        system.screen_size_px = [1280, 720];
        let mut debug = UiDrawList::new();
        debug.screen_size_px = [1280, 720];
        layers.push(newengine_ui_api::UiLayerDrawPacket::new(
            newengine_ui_api::UiLayerDomain::System,
            4,
            system,
        ));
        layers.push(newengine_ui_api::UiLayerDrawPacket::new(
            newengine_ui_api::UiLayerDomain::Debug,
            4,
            debug,
        ));
        let stats = ui_layer_payload_stats(&layers);
        assert_eq!(layers.packets.len(), 2);
        assert_eq!(stats.texture_bytes, 0);
    }
}
