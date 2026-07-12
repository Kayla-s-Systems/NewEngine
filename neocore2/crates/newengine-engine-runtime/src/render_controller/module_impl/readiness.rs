#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets::AssetServiceClient;
use newengine_core::render::{GpuResourceResidencyState, RenderApi};
use newengine_materials::api::MaterialRegistryApi;
use newengine_math::collections::FxHashSet;
use newengine_plugin_host::default_host_api;

use crate::gameplay::{clear_player_input, first_player, GameReadyWorldLaunchGate, GameRunMode};
use crate::scene_bridge::{PreparedTerrainPrimitiveMesh, TerrainSurfaceLayers};
use newengine_procedural_noise::ProceduralTerrain;

use super::super::material_bindings::{LitMaterialPlan, MaterialTextureGpuResidency};
use super::RuntimeRenderController;

const SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES: u64 = 1_800;
const SCENE_TEXTURE_GATE_SOFT_TIMEOUT_MS: u64 = 90_000;
const SCENE_TEXTURE_LAUNCH_MIN_RATIO_DEFAULT: f32 = 1.00;

/// Updates the standalone game launch gate and returns whether the playable world
/// may be simulated or possessed this frame.
///
/// CPU scene bootstrap can finish before GPU texture residency. We keep direct
/// player control and simulation closed while the renderer gets a short chance
/// to make declared material textures resident. This gate is deliberately soft:
/// missing optional backends or slow/failed texture uploads must never strand the
/// application on the loading projection. After the bounded wait expires the
/// runtime enters public Play and lets renderer fallbacks finish the frame.
pub(super) fn update_game_ready_launch_gate(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &mut newengine_ecs::World,
    _requested_play_mode: GameRunMode,
    frame_index: u64,
) -> bool {
    let Some(gate_snapshot) = world.resource::<GameReadyWorldLaunchGate>().cloned() else {
        return true;
    };

    if gate_snapshot.is_released() {
        return true;
    }

    let now_ms = wall_clock_millis();
    let readiness = critical_scene_residency_ready(this, r, world);
    let mut release: Option<(bool, u64, u64, String)> = None;

    if let Some(gate) = world.resource_mut::<GameReadyWorldLaunchGate>() {
        let first_wait = gate.requested_frame == u64::MAX;
        if first_wait {
            gate.requested_frame = frame_index;
        } else {
            gate.requested_frame = gate.requested_frame.min(frame_index);
        }
        if gate.requested_at_ms == 0 {
            gate.requested_at_ms = now_ms;
        }
        gate.update_texture_counts(readiness.waiting, readiness.total, readiness.failed);

        let waited_frames = frame_index.saturating_sub(gate.requested_frame);
        let waited_ms = now_ms.saturating_sub(gate.requested_at_ms);
        let soft_timeout = waited_frames >= scene_texture_gate_soft_timeout_frames()
            || waited_ms >= scene_texture_gate_soft_timeout_ms();

        if readiness.ready {
            gate.release(frame_index, readiness.reason);
            release = Some((false, waited_frames, waited_ms, gate.reason.clone()));
        } else if soft_timeout {
            let fallback_reason = format!(
                "soft timeout released with renderer fallbacks waited_ms={waited_ms} waited_frames={waited_frames} waiting={} total={} failed={} last='{}'",
                readiness.waiting, readiness.total, readiness.failed, readiness.reason
            );
            gate.release(frame_index, fallback_reason);
            release = Some((true, waited_frames, waited_ms, gate.reason.clone()));
        } else {
            gate.reason = readiness.reason;
            let early_wait_frame = waited_frames <= 8;
            if first_wait || frame_index.is_multiple_of(60) {
                newengine_ulog_api::ulog::info!(
                    "game-ready launch gate: blocked frame={} waited_ms={} reason='{}'",
                    frame_index,
                    waited_ms,
                    gate.reason
                );
            } else if early_wait_frame {
                newengine_ulog_api::ulog::debug!(
                    "game-ready launch gate: blocked frame={} waited_ms={} reason='{}'",
                    frame_index,
                    waited_ms,
                    gate.reason
                );
            }
        }
    } else {
        return true;
    }

    if let Some((fallback, waited_frames, waited_ms, reason)) = release {
        if fallback {
            newengine_ulog_api::ulog::warn!(
                "game-ready launch gate: soft-timeout release frame={} waited_frames={} waited_ms={} reason='{}'",
                frame_index,
                waited_frames,
                waited_ms,
                reason
            );
            newengine_core::crash::record_breadcrumb(format!(
                "game-ready launch gate: fallback release frame={} waited_ms={} reason='{}'",
                frame_index, waited_ms, reason
            ));
        } else {
            newengine_ulog_api::ulog::info!(
                "game-ready launch gate: released frame={} waited_frames={} waited_ms={} reason='{}'",
                frame_index,
                waited_frames,
                waited_ms,
                reason
            );
        }
        return true;
    }

    if let Some(player) = first_player(world) {
        clear_player_input(world, player);
    }
    false
}

#[derive(Clone, Debug)]
struct LaunchReadiness {
    ready: bool,
    reason: String,
    waiting: u32,
    total: u32,
    failed: u32,
}

fn critical_scene_residency_ready(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &newengine_ecs::World,
) -> LaunchReadiness {
    let static_world = critical_static_world_ready(world);
    let primitive_gpu = critical_primitive_gpu_ready(this, world);
    let materials = critical_scene_materials_ready(this, r, world);
    let terrain = critical_terrain_gpu_ready(this, world);

    let reason = if !static_world.ready {
        static_world.reason.clone()
    } else if !primitive_gpu.ready {
        primitive_gpu.reason.clone()
    } else if !materials.ready {
        materials.reason.clone()
    } else if !terrain.ready {
        terrain.reason.clone()
    } else {
        format!(
            "{} · {} · {} · {}",
            static_world.reason, primitive_gpu.reason, materials.reason, terrain.reason
        )
    };
    LaunchReadiness {
        ready: static_world.ready && primitive_gpu.ready && materials.ready && terrain.ready,
        reason,
        waiting: static_world
            .waiting
            .saturating_add(primitive_gpu.waiting)
            .saturating_add(materials.waiting)
            .saturating_add(terrain.waiting),
        total: static_world
            .total
            .saturating_add(primitive_gpu.total)
            .saturating_add(materials.total)
            .saturating_add(terrain.total),
        failed: static_world
            .failed
            .saturating_add(primitive_gpu.failed)
            .saturating_add(materials.failed)
            .saturating_add(terrain.failed),
    }
}

fn critical_primitive_gpu_ready(
    this: &RuntimeRenderController,
    world: &newengine_ecs::World,
) -> LaunchReadiness {
    let mut unique = std::collections::BTreeSet::new();
    for (_entity, primitive) in world.query::<newengine_primitives::Primitive>() {
        unique.insert(primitive.id);
    }
    let total = unique.len() as u32;
    if total == 0 {
        return LaunchReadiness {
            ready: true,
            reason: "no primitive gpu meshes declared".to_owned(),
            waiting: 0,
            total: 0,
            failed: 0,
        };
    }
    let resident = unique
        .iter()
        .filter(|id| this.gpu.meshes.prim_cache.contains_key(*id))
        .count() as u32;
    let waiting = total.saturating_sub(resident);
    LaunchReadiness {
        ready: waiting == 0,
        reason: if waiting == 0 {
            format!("primitive gpu meshes resident ready={resident}/{total}")
        } else {
            format!("waiting for bounded primitive gpu residency ready={resident}/{total} waiting={waiting}")
        },
        waiting,
        total,
        failed: 0,
    }
}

fn critical_static_world_ready(world: &newengine_ecs::World) -> LaunchReadiness {
    let Some(residency) = world.resource::<crate::scene_bridge::GameReadyStaticWorldResidency>()
    else {
        return LaunchReadiness {
            ready: true,
            reason: "no incremental static world declared".to_owned(),
            waiting: 0,
            total: 0,
            failed: 0,
        };
    };
    LaunchReadiness {
        ready: residency.is_ready(),
        reason: if residency.is_ready() {
            format!(
                "static world assembled completed={}/{} failed={}",
                residency.completed(),
                residency.total(),
                residency.failed(),
            )
        } else {
            format!(
                "waiting for incremental static world completed={}/{} pending={} failed={}",
                residency.completed(),
                residency.total(),
                residency.pending(),
                residency.failed(),
            )
        },
        waiting: residency.pending(),
        total: residency.total(),
        failed: residency.failed(),
    }
}

fn critical_terrain_gpu_ready(
    this: &RuntimeRenderController,
    world: &newengine_ecs::World,
) -> LaunchReadiness {
    let mut prepared_total = 0_u32;
    let mut resident = 0_u32;
    let mut waiting = 0_u32;
    let mut declared_total = 0_u32;

    for (entity, terrain) in world.query::<ProceduralTerrain>() {
        declared_total = declared_total.saturating_add(1);
        // Launch readiness must not wait on every future/prospective streamed
        // chunk. Only terrain that has crossed the CPU RenderPrep boundary is
        // launch-critical; the rest remains normal streaming work.
        if world.get::<PreparedTerrainPrimitiveMesh>(entity).is_none() {
            continue;
        }

        prepared_total = prepared_total.saturating_add(1);
        if this
            .gpu
            .meshes
            .terrain_cache
            .contains_key(&terrain.mesh_key())
        {
            resident = resident.saturating_add(1);
        } else {
            waiting = waiting.saturating_add(1);
        }
    }

    if prepared_total == 0 {
        return LaunchReadiness {
            ready: declared_total == 0,
            reason: if declared_total == 0 {
                "no terrain packets declared".to_owned()
            } else {
                format!("waiting for terrain RenderPrep packets declared={declared_total}")
            },
            waiting: declared_total,
            total: declared_total,
            failed: 0,
        };
    }

    let min_ready =
        crate::env_config::var_u32("NEWENGINE_TERRAIN_LAUNCH_MIN_READY_PACKETS", 1, 1, 64)
            .min(prepared_total);

    if resident >= min_ready {
        LaunchReadiness {
            ready: true,
            reason: format!(
                "terrain launch packets resident ready={resident}/{prepared_total} declared={declared_total} min_ready={min_ready}"
            ),
            waiting,
            total: prepared_total,
            failed: 0,
        }
    } else {
        LaunchReadiness {
            ready: false,
            reason: format!(
                "waiting for first terrain GPU packets resident={resident}/{prepared_total} declared={declared_total} min_ready={min_ready}"
            ),
            waiting,
            total: prepared_total,
            failed: 0,
        }
    }
}

fn critical_scene_materials_ready(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &newengine_ecs::World,
) -> LaunchReadiness {
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();
    let mut unique_paths = FxHashSet::<String>::default();
    let mut alpha_critical_paths = FxHashSet::<String>::default();

    for (_entity, material_ref) in world.query::<newengine_materials::MaterialRef>() {
        let resolved = mats.resolve(material_ref.id);
        let plan = LitMaterialPlan::from_resolved(resolved.as_ref(), [1.0, 1.0, 1.0, 1.0]);
        if plan.alpha_cutoff > 0.0 {
            if let Some(path) = plan.base_color_texture {
                alpha_critical_paths.insert(path.to_owned());
            }
        }
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

    for (_entity, layers) in world.query::<TerrainSurfaceLayers>() {
        unique_paths.insert(layers.forest_base_texture.clone());
        unique_paths.insert(layers.sand_base_texture.clone());
        unique_paths.insert(layers.rock_base_texture.clone());
    }

    let mut total = 0_u32;
    let mut waiting = 0_u32;
    let mut failed = 0_u32;
    let mut optional = 0_u32;
    let mut alpha_waiting = 0_u32;
    let mut alpha_failed = 0_u32;
    for path in unique_paths.iter() {
        this.request_material_texture(path);
        if is_launch_gate_optional_texture(path) {
            optional = optional.saturating_add(1);
            continue;
        }
        total = total.saturating_add(1);
        let alpha_critical = alpha_critical_paths.contains(path);
        match material_texture_ready_state(this, r, path) {
            TextureReadyState::Ready => {}
            TextureReadyState::Failed => {
                failed = failed.saturating_add(1);
                if alpha_critical {
                    alpha_failed = alpha_failed.saturating_add(1);
                }
            }
            TextureReadyState::Waiting => {
                waiting = waiting.saturating_add(1);
                if alpha_critical {
                    alpha_waiting = alpha_waiting.saturating_add(1);
                }
            }
        }
    }

    if total == 0 {
        return LaunchReadiness {
            ready: true,
            reason: if optional == 0 {
                "no critical scene textures declared".to_owned()
            } else {
                format!("only optional environment textures declared optional={optional}")
            },
            waiting: 0,
            total,
            failed: 0,
        };
    }

    let ready_count = total.saturating_sub(waiting).saturating_sub(failed);
    let configured_min_ready = crate::env_config::var_u32(
        "NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY",
        total,
        0,
        total.max(1),
    )
    .min(total);
    let visual_floor = scene_texture_launch_visual_floor(total);
    let min_ready = configured_min_ready.max(visual_floor).min(total);

    if alpha_waiting > 0 || alpha_failed > 0 {
        LaunchReadiness {
            ready: false,
            reason: format!(
                "waiting for alpha-critical texture residency ready={}/{} waiting={} failed={} policy='Masked base textures never use opaque fallback'",
                alpha_critical_paths.len() as u32 - alpha_waiting - alpha_failed,
                alpha_critical_paths.len(),
                alpha_waiting,
                alpha_failed,
            ),
            waiting,
            total,
            failed,
        }
    } else if waiting == 0 {
        let suffix = if failed == 0 {
            format!("scene material textures ready total={total}")
        } else {
            format!("scene material textures ready with fallbacks total={total} failed={failed}")
        };
        LaunchReadiness {
            ready: true,
            reason: suffix,
            waiting,
            total,
            failed,
        }
    } else if min_ready > 0 && ready_count >= min_ready {
        LaunchReadiness {
            ready: true,
            reason: format!(
                "scene material textures partially resident ready={ready_count}/{total} waiting={waiting} failed={failed} min_ready={min_ready} policy='NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY/NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO'"
            ),
            waiting,
            total,
            failed,
        }
    } else {
        LaunchReadiness {
            ready: false,
            reason: format!(
                "waiting for scene texture residency ready={ready_count}/{total} waiting={waiting} failed={failed} min_ready={min_ready}"
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

const LAUNCH_OPTIONAL_TEXTURE_TAGS: &[&str] = &["sky", "skydome", "cloud", "moon"];

#[inline]
fn is_launch_gate_optional_texture(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    LAUNCH_OPTIONAL_TEXTURE_TAGS
        .iter()
        .any(|tag| lower.contains(tag))
}

fn scene_texture_gate_soft_timeout_frames() -> u64 {
    crate::env_config::var_u64(
        "NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES",
        SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES,
        60,
        18_000,
    )
}

fn scene_texture_gate_soft_timeout_ms() -> u64 {
    crate::env_config::var_u64(
        "NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_MS",
        SCENE_TEXTURE_GATE_SOFT_TIMEOUT_MS,
        5_000,
        600_000,
    )
}

#[inline]
fn wall_clock_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

fn scene_texture_launch_visual_floor(total: u32) -> u32 {
    if total <= 1 {
        return total;
    }
    let ratio = crate::env_config::var_f32(
        "NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO",
        SCENE_TEXTURE_LAUNCH_MIN_RATIO_DEFAULT,
        0.50,
        1.00,
    );
    ((total as f32) * ratio).ceil() as u32
}

fn material_texture_ready_state(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    path: &str,
) -> TextureReadyState {
    let Some(entry) = this.gpu.material.textures.get(path).cloned() else {
        return TextureReadyState::Waiting;
    };

    match entry {
        MaterialTextureGpuResidency::Ready { .. } => TextureReadyState::Ready,
        MaterialTextureGpuResidency::Failed { .. } => TextureReadyState::Failed,
        MaterialTextureGpuResidency::Requested
        | MaterialTextureGpuResidency::AssetLoading { .. }
        | MaterialTextureGpuResidency::CpuDecoding { .. } => TextureReadyState::Waiting,
        MaterialTextureGpuResidency::GpuLoading { texture, .. } => {
            match r.texture_residency(texture) {
                Ok(snapshot) if snapshot.state == GpuResourceResidencyState::Ready => {
                    this.gpu.material.textures.insert(
                        path.to_owned(),
                        MaterialTextureGpuResidency::Ready { texture },
                    );
                    let assets = AssetServiceClient::new(default_host_api());
                    let _ = assets.project_status_json_v1(serde_json::json!({
                        "owner": "render.launch_gate",
                        "domain": "gpu",
                        "logical_path": path,
                        "stage": "resident",
                        "state": "ready",
                        "resource_id": format!("{:?}", texture),
                        "proof": {
                            "texture": format!("{:?}", texture),
                            "residency": "ready"
                        },
                        "detail": "GPU texture residency confirmed by scene launch gate"
                    }));
                    TextureReadyState::Ready
                }
                Ok(snapshot) if snapshot.state == GpuResourceResidencyState::Failed => {
                    let message = snapshot
                        .message
                        .unwrap_or_else(|| "gpu upload failed".to_owned());
                    newengine_ulog_api::ulog::warn!(
                        "game-ready launch gate: material texture failed path='{}' err='{}'",
                        path,
                        message
                    );
                    this.gpu.material.textures.insert(
                        path.to_owned(),
                        MaterialTextureGpuResidency::Failed { message },
                    );
                    TextureReadyState::Failed
                }
                Err(e) => {
                    let message = e.to_string();
                    newengine_ulog_api::ulog::warn!(
                        "game-ready launch gate: material texture residency query failed path='{}' err='{}'",
                        path,
                        message
                    );
                    this.gpu.material.textures.insert(
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
