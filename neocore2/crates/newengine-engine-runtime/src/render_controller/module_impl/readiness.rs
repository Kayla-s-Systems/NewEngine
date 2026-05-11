#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{GpuResourceResidencyState, RenderApi};
use newengine_materials::api::MaterialRegistryApi;
use newengine_procedural_noise::ProceduralTerrain;

use crate::gameplay::{clear_player_input, first_player, EditorPlayMode, GameReadyWorldLaunchGate};

use super::super::material_bindings::LitMaterialPlan;
use super::RuntimeRenderController;

/// Updates the standalone game launch gate and returns whether the playable world
/// may be simulated/rendered this frame.
///
/// CPU scene bootstrap can finish before GPU texture residency. We keep direct
/// player control, simulation and terrain drawing closed until critical terrain
/// material textures are resident or permanently failed. Failed textures release
/// the gate intentionally: at that point the runtime may use explicit fallbacks
/// instead of waiting forever.
pub(super) fn update_game_ready_launch_gate(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &mut newengine_ecs::World,
    requested_play_mode: EditorPlayMode,
    frame_index: u64,
) -> bool {
    let Some(gate_snapshot) = world.resource::<GameReadyWorldLaunchGate>().cloned() else {
        return true;
    };

    if gate_snapshot.released || !requested_play_mode.wants_direct_player_control() {
        return true;
    }

    let readiness = critical_terrain_materials_ready(this, r, world);
    if readiness.ready {
        let (waited_frames, reason) = {
            let Some(gate) = world.resource_mut::<GameReadyWorldLaunchGate>() else {
                return true;
            };
            gate.requested_frame = gate.requested_frame.min(frame_index);
            gate.release(frame_index, readiness.reason);
            (frame_index.saturating_sub(gate.requested_frame), gate.reason.clone())
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
            gate.requested_frame = gate.requested_frame.min(frame_index);
            gate.reason = readiness.reason;
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
}

fn critical_terrain_materials_ready(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &newengine_ecs::World,
) -> LaunchReadiness {
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();
    let mut total = 0u32;
    let mut waiting = 0u32;

    for (entity, terrain) in world.query::<ProceduralTerrain>() {
        let resolved = world
            .get::<newengine_materials::MaterialRef>(entity)
            .and_then(|mr| mats.resolve(mr.id));
        let plan = LitMaterialPlan::from_resolved(resolved.as_ref(), terrain.base_color);
        for path in [
            plan.base_color_texture,
            plan.normal_texture,
            plan.roughness_texture,
        ]
        .into_iter()
        .flatten()
        {
            total = total.saturating_add(1);
            this.request_material_texture(path);
            if !this.material_texture_ready_or_failed(r, path) {
                waiting = waiting.saturating_add(1);
            }
        }
    }

    if total == 0 {
        return LaunchReadiness {
            ready: true,
            reason: "no critical terrain textures declared".to_owned(),
        };
    }

    if waiting == 0 {
        LaunchReadiness {
            ready: true,
            reason: format!("critical terrain textures ready total={total}"),
        }
    } else {
        LaunchReadiness {
            ready: false,
            reason: format!("waiting for terrain texture residency waiting={waiting} total={total}"),
        }
    }
}
