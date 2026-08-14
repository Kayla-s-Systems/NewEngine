use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_transform::Transform;

use newengine_engine_runtime::gameplay::{
    first_player, DisplayMode, DisplayVisibility, Health, PhysicsBodyDesc,
};
use newengine_gameplay_fps_api::{
    FpsDemoGoal, FpsDemoHazard, FpsDemoPickup, FpsDemoState, FpsDemoTarget,
    FpsGameplayPolicyProvider, FpsGameplayPolicySnapshot, FpsPolicyDecision, FpsPolicyEvent,
};
use newengine_gameplay_script_runtime::GameplayCommandExecutor;

use crate::script_commands::execute_policy_commands;

#[inline]
fn distance_sq(a: Vec3, b: Vec3) -> f32 {
    let d = a - b;
    d.length_squared()
}

pub fn step_fps_demo_gameplay(
    world: &mut World,
    dt: f32,
    policy_provider: &dyn FpsGameplayPolicyProvider,
    command_executor: &GameplayCommandExecutor,
) {
    if world.resource::<FpsDemoState>().is_none() {
        return;
    }
    let Some(policy) = world.resource::<FpsGameplayPolicySnapshot>().cloned() else {
        // Production FPS content installs this snapshot before gameplay is admitted.
        return;
    };

    let terminal = world
        .resource::<FpsDemoState>()
        .map(|s| s.completed || s.failed)
        .unwrap_or(false);

    if !terminal {
        if let Some(state) = world.resource_mut::<FpsDemoState>() {
            if dt.is_finite() && dt > 0.0 {
                state.elapsed_sec += dt.min(0.1);
            }
        }
    }

    let Some(player) = first_player(world) else {
        return;
    };
    let Some(player_pos) = world.get::<Transform>(player).map(|t| t.position) else {
        return;
    };

    if terminal {
        return;
    }

    let mut picked: Vec<EntityId> = Vec::new();
    for (entity, pickup) in world.query::<FpsDemoPickup>() {
        let Some(t) = world.get::<Transform>(entity) else {
            continue;
        };
        let r = pickup.radius.max(0.1);
        if distance_sq(player_pos, t.position) <= r * r {
            picked.push(entity);
        }
    }
    picked.sort_by_key(|id| id.stable_u64());

    for entity in &picked {
        let _ = world.remove::<FpsDemoPickup>(*entity);
        let _ = world.insert(
            *entity,
            DisplayVisibility {
                mode: DisplayMode::RuntimeHidden,
            },
        );
    }

    let mut destroyed_targets: Vec<EntityId> = Vec::new();
    for (entity, _) in world.query::<FpsDemoTarget>() {
        if world
            .get::<Health>(entity)
            .is_some_and(|health| !health.alive())
        {
            destroyed_targets.push(entity);
        }
    }
    destroyed_targets.sort_by_key(|id| id.stable_u64());
    for entity in &destroyed_targets {
        let _ = world.remove::<FpsDemoTarget>(*entity);
        let _ = world.remove::<PhysicsBodyDesc>(*entity);
        let _ = world.insert(
            *entity,
            DisplayVisibility {
                mode: DisplayMode::RuntimeHidden,
            },
        );
    }

    let hit_hazard = world.query::<FpsDemoHazard>().any(|(entity, hazard)| {
        world.get::<Transform>(entity).is_some_and(|t| {
            let r = hazard.radius.max(0.1);
            distance_sq(player_pos, t.position) <= r * r
        })
    });

    let reached_goal = world.query::<FpsDemoGoal>().any(|(entity, goal)| {
        world.get::<Transform>(entity).is_some_and(|t| {
            let r = goal.radius.max(0.1);
            distance_sq(player_pos, t.position) <= r * r
        })
    });

    let collected_delta = picked.len() as u32;
    let destroyed_delta = destroyed_targets.len() as u32;

    let event = if let Some(state) = world.resource_mut::<FpsDemoState>() {
        state.pickups_collected = state
            .pickups_collected
            .saturating_add(collected_delta)
            .min(state.pickups_total);
        state.targets_destroyed = state
            .targets_destroyed
            .saturating_add(destroyed_delta)
            .min(state.targets_total);
        FpsPolicyEvent::Mission {
            pickups_collected: state.pickups_collected,
            pickups_total: state.pickups_total,
            targets_destroyed: state.targets_destroyed,
            targets_total: state.targets_total,
            collected_delta,
            destroyed_delta,
            hit_hazard,
            reached_goal,
        }
    } else {
        return;
    };

    let event_happened = collected_delta > 0 || destroyed_delta > 0 || hit_hazard || reached_goal;
    let mut decision = if event_happened && !policy.callbacks.mission_event.trim().is_empty() {
        match policy_provider.invoke_event(&policy.callbacks.mission_event, &event) {
            Ok(decision) => decision,
            Err(error) => {
                newengine_ulog_api::ulog::error!(
                    "fps Lua mission callback failed export='{}' err='{}'; policy='fail closed: no scripted transition this tick'",
                    policy.callbacks.mission_event,
                    error
                );
                FpsPolicyDecision {
                    allow_default: false,
                    status: Some(format!("Gameplay script error: {error}")),
                    ..FpsPolicyDecision::default()
                }
            }
        }
    } else {
        FpsPolicyDecision::default()
    };

    if let Err(error) =
        execute_policy_commands(world, command_executor, &decision.commands, "mission")
    {
        decision.allow_default = false;
        decision.completed = None;
        decision.failed = None;
        decision.status = Some(format!("Gameplay command transaction failed: {error}"));
    }

    if let Some(state) = world.resource_mut::<FpsDemoState>() {
        if decision.allow_default {
            apply_default_mission_policy(
                state,
                &policy,
                collected_delta,
                destroyed_delta,
                hit_hazard,
                reached_goal,
            );
        }
        if let Some(completed) = decision.completed {
            state.completed = completed;
        }
        if let Some(failed) = decision.failed {
            state.failed = failed;
        }
        if let Some(status) = decision.status {
            state.status = status;
        } else if !event_happened && !state.completed && !state.failed {
            state.status = policy.mission.default_status.clone();
        }
    }
}

fn apply_default_mission_policy(
    state: &mut FpsDemoState,
    policy: &FpsGameplayPolicySnapshot,
    collected_delta: u32,
    destroyed_delta: u32,
    hit_hazard: bool,
    reached_goal: bool,
) {
    let mission = &policy.mission;
    let pickups_complete =
        !mission.require_pickups || state.pickups_collected >= state.pickups_total;
    let targets_complete =
        !mission.require_targets || state.targets_destroyed >= state.targets_total;
    let objectives_complete = pickups_complete && targets_complete;

    if hit_hazard && mission.hazard_fails {
        state.failed = true;
        state.status = mission.hazard_status.clone();
    } else if reached_goal && (!mission.goal_requires_objectives || objectives_complete) {
        state.completed = true;
        state.status = mission.goal_complete_status.clone();
    } else if reached_goal {
        state.status = mission.goal_locked_status.clone();
    } else if destroyed_delta > 0 {
        state.status = mission.target_status.clone();
    } else if collected_delta > 0 {
        state.status = mission.pickup_status.clone();
    } else {
        state.status = mission.default_status.clone();
    }
}
