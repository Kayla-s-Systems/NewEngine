#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets::AssetServiceClient;
use newengine_core::render::{GpuResourceResidencyState, RenderApi};
use newengine_materials::api::MaterialRegistryApi;
use newengine_math::collections::FxHashSet;
use newengine_plugin_host::default_host_api;

use crate::gameplay::{clear_player_input, first_player, EditorPlayMode, GameReadyWorldLaunchGate};

use super::super::material_bindings::{LitMaterialPlan, MaterialTextureGpuResidency};
use super::RuntimeRenderController;

const SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES: u64 = 360;

/// Updates the standalone game launch gate and returns whether the playable world
/// may be simulated or possessed this frame.
///
/// CPU scene bootstrap can finish before GPU texture residency. We keep direct
/// player control and simulation closed until all declared scene material
/// textures are either resident or explicitly failed. Failed textures release the
/// gate intentionally: at that point the runtime uses renderer fallbacks instead
/// of waiting forever.
pub(super) fn update_game_ready_launch_gate(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &mut newengine_ecs::World,
    _requested_play_mode: EditorPlayMode,
    frame_index: u64,
) -> bool {
    let Some(gate_snapshot) = world.resource::<GameReadyWorldLaunchGate>().cloned() else {
        return true;
    };

    if gate_snapshot.is_released() {
        return true;
    }


    let readiness = critical_scene_materials_ready(this, r, world);
    if readiness.ready {
        let (waited_frames, reason) = {
            let Some(gate) = world.resource_mut::<GameReadyWorldLaunchGate>() else {
                return true;
            };
            gate.requested_frame = gate.requested_frame.min(frame_index);
            gate.update_texture_counts(readiness.waiting, readiness.total, readiness.failed);
            gate.release(frame_index, readiness.reason);
            (
                frame_index.saturating_sub(gate.requested_frame),
                gate.reason.clone(),
            )
        };
        log::info!(
            "game-ready launch gate: released frame={} waited_frames={} reason='{}'",
            frame_index,
            waited_frames,
            reason
        );
        true
    } else {
        if let Some(gate) = world.resource_mut::<GameReadyWorldLaunchGate>() {
            let first_wait = gate.requested_frame == u64::MAX;
            gate.requested_frame = gate.requested_frame.min(frame_index);
            gate.update_texture_counts(readiness.waiting, readiness.total, readiness.failed);
            let waited_frames = frame_index.saturating_sub(gate.requested_frame);

            if waited_frames >= SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES {
                let reason = format!(
                    "scene texture residency soft timeout after {waited_frames} frames; continuing with renderer fallbacks waiting={} total={} failed={}",
                    readiness.waiting,
                    readiness.total,
                    readiness.failed
                );
                gate.release(frame_index, reason.clone());
                log::warn!(
                    "game-ready launch gate: soft-released frame={} waited_frames={} reason='{}'",
                    frame_index,
                    waited_frames,
                    reason
                );
                return true;
            }

            gate.reason = readiness.reason;
            let early_wait_frame = waited_frames <= 8;
            if first_wait || early_wait_frame || frame_index % 60 == 0 {
                log::info!(
                    "game-ready launch gate: blocked frame={} reason='{}'",
                    frame_index,
                    gate.reason
                );
            }
        }
        if let Some(player) = first_player(world) {
            clear_player_input(world, player);
        }
        false
    }
}

#[derive(Clone, Debug)]
struct LaunchReadiness {
    ready: bool,
    reason: String,
    waiting: u32,
    total: u32,
    failed: u32,
}

fn critical_scene_materials_ready(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &newengine_ecs::World,
) -> LaunchReadiness {
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();
    let mut unique_paths = FxHashSet::<String>::default();

    for (_entity, material_ref) in world.query::<newengine_materials::MaterialRef>() {
        let resolved = mats.resolve(material_ref.id);
        let plan = LitMaterialPlan::from_resolved(resolved.as_ref(), [1.0, 1.0, 1.0, 1.0]);
        for path in [
            plan.base_color_texture,
            plan.normal_texture,
            plan.roughness_texture,
        ]
        .into_iter()
        .flatten()
        {
            unique_paths.insert(path.to_owned());
        }
    }

    let total = unique_paths.len() as u32;
    if total == 0 {
        return LaunchReadiness {
            ready: true,
            reason: "no critical scene textures declared".to_owned(),
            waiting: 0,
            total,
            failed: 0,
        };
    }

    let mut waiting = 0_u32;
    let mut failed = 0_u32;
    for path in unique_paths.iter() {
        this.request_material_texture(path);
        match material_texture_ready_state(this, r, path) {
            TextureReadyState::Ready => {}
            TextureReadyState::Failed => failed = failed.saturating_add(1),
            TextureReadyState::Waiting => waiting = waiting.saturating_add(1),
        }
    }

    if waiting == 0 {
        let suffix = if failed == 0 {
            format!("scene material textures ready total={total}")
        } else {
            format!(
                "scene material textures ready with fallbacks total={total} failed={failed}"
            )
        };
        LaunchReadiness {
            ready: true,
            reason: suffix,
            waiting,
            total,
            failed,
        }
    } else {
        LaunchReadiness {
            ready: false,
            reason: format!(
                "waiting for scene texture residency waiting={waiting} total={total} failed={failed}"
            ),
            waiting,
            total,
            failed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextureReadyState {
    Ready,
    Waiting,
    Failed,
}

fn material_texture_ready_state(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    path: &str,
) -> TextureReadyState {
    let Some(entry) = this.material_textures.get(path).cloned() else {
        return TextureReadyState::Waiting;
    };

    match entry {
        MaterialTextureGpuResidency::Ready { .. } => TextureReadyState::Ready,
        MaterialTextureGpuResidency::Failed { .. } => TextureReadyState::Failed,
        MaterialTextureGpuResidency::Requested
        | MaterialTextureGpuResidency::AssetLoading { .. } => TextureReadyState::Waiting,
        MaterialTextureGpuResidency::GpuLoading { texture, .. } => {
            match r.texture_residency(texture) {
                Ok(snapshot) if snapshot.state == GpuResourceResidencyState::Ready => {
                    this.material_textures.insert(
                        path.to_owned(),
                        MaterialTextureGpuResidency::Ready { texture },
                    );
                    let assets = AssetServiceClient::new(default_host_api());
                    let _ = assets.mark_status_json_v1(serde_json::json!({
                        "logical_path": path,
                        "stage": "resident",
                        "state": "ready",
                        "source": "render.launch_gate",
                        "detail": "GPU texture residency confirmed by scene launch gate"
                    }));
                    TextureReadyState::Ready
                }
                Ok(snapshot) if snapshot.state == GpuResourceResidencyState::Failed => {
                    let message = snapshot
                        .message
                        .unwrap_or_else(|| "gpu upload failed".to_owned());
                    log::warn!(
                        "game-ready launch gate: material texture failed path='{}' err='{}'",
                        path,
                        message
                    );
                    this.material_textures.insert(
                        path.to_owned(),
                        MaterialTextureGpuResidency::Failed { message },
                    );
                    TextureReadyState::Failed
                }
                Err(e) => {
                    let message = e.to_string();
                    log::warn!(
                        "game-ready launch gate: material texture residency query failed path='{}' err='{}'",
                        path,
                        message
                    );
                    this.material_textures.insert(
                        path.to_owned(),
                        MaterialTextureGpuResidency::Failed { message },
                    );
                    TextureReadyState::Failed
                }
                _ => TextureReadyState::Waiting,
            }
        }
    }
}
