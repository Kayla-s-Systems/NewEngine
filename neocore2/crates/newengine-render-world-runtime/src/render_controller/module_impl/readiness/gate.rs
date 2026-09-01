use std::sync::OnceLock;
use std::time::Instant;

use newengine_materials::api::MaterialRegistryApi;

use newengine_gameplay_world_runtime::gameplay::{
    clear_player_input, first_player, GameRunMode, PhysicsStaticColliderSyncProgress,
    PlayerModelAssignment, PlayerModelBinding, StaticMeshCollider, WorldActivationState,
    WorldAssemblyProgress,
};

use super::super::super::material_bindings::MaterialTextureGpuResidency;
use super::super::RuntimeRenderController;
use super::materials::{cached_scene_material_launch_plan, SceneMaterialLaunchPlan};
use super::residency::critical_scene_residency_ready;

static SCENE_LAUNCH_EPOCH: OnceLock<Instant> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlayerVisualReadiness {
    pending: bool,
    reason: String,
}

fn required_player_visual_readiness(world: &newengine_ecs::World) -> PlayerVisualReadiness {
    let Some(player) = first_player(world) else {
        return PlayerVisualReadiness {
            pending: false,
            reason: String::new(),
        };
    };
    let Some(assignment) = world.get::<PlayerModelAssignment>(player) else {
        return PlayerVisualReadiness {
            pending: false,
            reason: String::new(),
        };
    };
    if !assignment.enabled || assignment.source.trim().is_empty() {
        return PlayerVisualReadiness {
            pending: false,
            reason: String::new(),
        };
    }

    let binding = world.get::<PlayerModelBinding>(player);
    let ready = binding.is_some_and(|binding| {
        binding.assignment_revision == assignment.revision
            && binding.source == assignment.source
            && binding.visual_root.is_some()
            && binding.part_count > 0
    });
    if ready {
        return PlayerVisualReadiness {
            pending: false,
            reason: String::new(),
        };
    }

    let detail = binding.map_or_else(
        || "binding=missing".to_owned(),
        |binding| {
            format!(
                "binding_revision={} assignment_revision={} source_match={} visual_root={} parts={}",
                binding.assignment_revision,
                assignment.revision,
                binding.source == assignment.source,
                binding.visual_root.is_some(),
                binding.part_count
            )
        },
    );
    PlayerVisualReadiness {
        pending: true,
        reason: format!(
            "waiting for required playable-character visual binding player={} source='{}' {}",
            player.stable_u64(),
            assignment.source,
            detail
        ),
    }
}

pub(in crate::render_controller::module_impl) fn prepare_scene_launch_resources(
    this: &mut RuntimeRenderController,
    world: &mut newengine_ecs::World,
    materials: &dyn MaterialRegistryApi,
) -> SceneMaterialLaunchPlan {
    let plan = cached_scene_material_launch_plan(world, materials);
    for path in &plan.critical_paths {
        if plan.launch_required_paths.contains(path) && plan.visible_world_paths.contains(path) {
            this.prioritize_material_texture(path);
        } else if plan.launch_required_paths.contains(path)
            || plan.fallback_forbidden_paths.contains(path)
        {
            this.prioritize_player_weapon_texture(path);
        } else {
            this.request_material_texture_with_priority(
                path,
                super::super::super::state::MaterialTexturePriority::streaming_visible(),
            );
        }
    }
    for path in &plan.optional_paths {
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

fn launch_required_texture_pending(
    this: &RuntimeRenderController,
    material_plan: Option<&SceneMaterialLaunchPlan>,
) -> bool {
    let Some(plan) = material_plan else {
        return false;
    };
    plan.launch_required_paths.iter().any(|path| {
        !matches!(
            this.gpu.material.textures.get(path),
            Some(MaterialTextureGpuResidency::Ready { .. })
        )
    })
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
    // A soft renderer timeout may substitute textures/meshes with fallbacks, but it must
    // never release gameplay while authored world assembly (especially collision) is pending.
    let authored_world_pending = world
        .resource::<WorldAssemblyProgress>()
        .map(|progress| !progress.is_ready())
        .unwrap_or(false);
    let static_collision_total = world
        .query::<StaticMeshCollider>()
        .count()
        .min(u32::MAX as usize) as u32;
    let static_collision_declared = static_collision_total > 0;
    let physics_collision_pending = static_collision_declared
        && world
            .resource::<PhysicsStaticColliderSyncProgress>()
            .map(|progress| !progress.is_ready() || progress.registered < static_collision_total)
            .unwrap_or(true);
    let player_visual = required_player_visual_readiness(world);
    let launch_required_texture_pending = launch_required_texture_pending(this, material_plan);
    let launch_critical_pending = authored_world_pending
        || physics_collision_pending
        || player_visual.pending
        || launch_required_texture_pending;
    let critical_reason = if player_visual.pending {
        Some(player_visual.reason.as_str())
    } else if authored_world_pending {
        Some("waiting for authored world assembly")
    } else if physics_collision_pending {
        Some("waiting for authored static collision residency")
    } else if launch_required_texture_pending {
        Some("waiting for launch-critical texture working set")
    } else {
        None
    };
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

        if readiness.ready && !launch_critical_pending {
            gate.mark_ready(frame_index, readiness.reason);
            release = Some((false, waited_frames, waited_ms, gate.reason.clone()));
        } else if soft_timeout && !launch_critical_pending {
            let fallback_reason = format!(
                "soft timeout released with renderer fallbacks waited_ms={waited_ms} waited_frames={waited_frames} waiting={} total={} failed={} last='{}'",
                readiness.waiting, readiness.total, readiness.failed, readiness.reason
            );
            gate.mark_ready(frame_index, fallback_reason);
            release = Some((true, waited_frames, waited_ms, gate.reason.clone()));
        } else {
            gate.reason = critical_reason
                .map(str::to_owned)
                .unwrap_or(readiness.reason);
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
    newengine_runtime_policy::streaming_policy().scene_texture_gate_soft_timeout_frames
}

fn scene_texture_gate_soft_timeout_ms() -> u64 {
    newengine_runtime_policy::streaming_policy().scene_texture_gate_soft_timeout_ms
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
