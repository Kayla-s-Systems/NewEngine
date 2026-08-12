use std::sync::OnceLock;
use std::time::Instant;

use newengine_materials::api::MaterialRegistryApi;

use crate::gameplay::{clear_player_input, first_player, GameRunMode, WorldActivationState};

use super::super::RuntimeRenderController;
use super::materials::{build_scene_material_launch_plan, SceneMaterialLaunchPlan};
use super::residency::critical_scene_residency_ready;

const SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES: u64 = 1_800;
const SCENE_TEXTURE_GATE_SOFT_TIMEOUT_MS: u64 = 90_000;
static SCENE_LAUNCH_EPOCH: OnceLock<Instant> = OnceLock::new();

pub(in crate::render_controller::module_impl) fn prepare_scene_launch_resources(
    this: &mut RuntimeRenderController,
    world: &newengine_ecs::World,
    materials: &dyn MaterialRegistryApi,
) -> SceneMaterialLaunchPlan {
    let plan = build_scene_material_launch_plan(world, materials);
    for path in &plan.critical_paths {
        this.request_material_texture(path);
    }
    plan
}

pub(in crate::render_controller::module_impl) fn update_world_activation_gate(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    world: &mut newengine_ecs::World,
    requested_play_mode: GameRunMode,
    frame_index: u64,
) -> bool {
    update_world_activation_gate_impl(this, r, world, requested_play_mode, None, frame_index)
}

pub(in crate::render_controller::module_impl) fn update_world_activation_gate_with_material_plan(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    world: &mut newengine_ecs::World,
    requested_play_mode: GameRunMode,
    material_plan: &SceneMaterialLaunchPlan,
    frame_index: u64,
) -> bool {
    update_world_activation_gate_impl(
        this,
        r,
        world,
        requested_play_mode,
        Some(material_plan),
        frame_index,
    )
}

fn update_world_activation_gate_impl(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    world: &mut newengine_ecs::World,
    _requested_play_mode: GameRunMode,
    material_plan: Option<&SceneMaterialLaunchPlan>,
    frame_index: u64,
) -> bool {
    let Some(gate_snapshot) = world.resource::<WorldActivationState>().cloned() else {
        return true;
    };

    if gate_snapshot.is_ready() {
        return true;
    }

    let now_ms = launch_gate_millis();
    let readiness = critical_scene_residency_ready(this, r, world, material_plan);
    let mut release: Option<(bool, u64, u64, String)> = None;

    if let Some(gate) = world.resource_mut::<WorldActivationState>() {
        let first_wait = gate.requested_frame == u64::MAX;
        if first_wait {
            gate.requested_frame = frame_index;
        } else {
            gate.requested_frame = gate.requested_frame.min(frame_index);
        }
        if gate.requested_at_ms == 0 {
            gate.requested_at_ms = now_ms;
        }
        gate.update_residency(readiness.waiting, readiness.total, readiness.failed);

        let waited_frames = frame_index.saturating_sub(gate.requested_frame);
        let waited_ms = now_ms.saturating_sub(gate.requested_at_ms);
        let soft_timeout = waited_frames >= scene_texture_gate_soft_timeout_frames()
            || waited_ms >= scene_texture_gate_soft_timeout_ms();

        if readiness.ready {
            gate.mark_ready(frame_index, readiness.reason);
            release = Some((false, waited_frames, waited_ms, gate.reason.clone()));
        } else if soft_timeout {
            let fallback_reason = format!(
                "soft timeout released with renderer fallbacks waited_ms={waited_ms} waited_frames={waited_frames} waiting={} total={} failed={} last='{}'",
                readiness.waiting, readiness.total, readiness.failed, readiness.reason
            );
            gate.mark_ready(frame_index, fallback_reason);
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
fn launch_gate_millis() -> u64 {
    (SCENE_LAUNCH_EPOCH
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64)
        .max(1)
}
