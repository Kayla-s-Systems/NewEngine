#![forbid(unsafe_op_in_unsafe_fn)]

//! Project-policy objective event bridge for reusable FPS worlds.
//! This crate detects neutral objective facts and dispatches them to the project callback.
//! It deliberately owns no mission success/failure/status policy and has no default mission logic.

mod script_commands;

use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_transform::Transform;

use newengine_engine_runtime::gameplay::{
    first_player, DisplayMode, DisplayVisibility, Health, PhysicsBodyDesc,
};
use newengine_gameplay_fps_api::{
    FpsGameplayPolicyProvider, FpsGameplayPolicySnapshot, FpsObjectiveGoal, FpsObjectiveHazard,
    FpsObjectivePickup, FpsObjectiveState, FpsObjectiveTarget, FpsPolicyDecision, FpsPolicyEvent,
};
use newengine_gameplay_script_runtime::GameplayCommandExecutor;

use crate::script_commands::execute_policy_commands;

#[inline]
fn distance_sq(a: Vec3, b: Vec3) -> f32 {
    let d = a - b;
    d.length_squared()
}

pub fn step_fps_objective_events(
    world: &mut World,
    dt: f32,
    policy_provider: &dyn FpsGameplayPolicyProvider,
    command_executor: &GameplayCommandExecutor,
) {
    if world.resource::<FpsObjectiveState>().is_none() {
        return;
    }
    let Some(policy) = world.resource::<FpsGameplayPolicySnapshot>().cloned() else {
        // The active project installs this policy snapshot before objective events are admitted.
        return;
    };

    let terminal = world
        .resource::<FpsObjectiveState>()
        .map(|s| s.completed || s.failed)
        .unwrap_or(false);

    if !terminal {
        if let Some(state) = world.resource_mut::<FpsObjectiveState>() {
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
    for (entity, pickup) in world.query::<FpsObjectivePickup>() {
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
        let _ = world.remove::<FpsObjectivePickup>(*entity);
        let _ = world.insert(
            *entity,
            DisplayVisibility {
                mode: DisplayMode::RuntimeHidden,
            },
        );
    }

    let mut destroyed_targets: Vec<EntityId> = Vec::new();
    for (entity, _) in world.query::<FpsObjectiveTarget>() {
        if world
            .get::<Health>(entity)
            .is_some_and(|health| !health.alive())
        {
            destroyed_targets.push(entity);
        }
    }
    destroyed_targets.sort_by_key(|id| id.stable_u64());
    for entity in &destroyed_targets {
        let _ = world.remove::<FpsObjectiveTarget>(*entity);
        let _ = world.remove::<PhysicsBodyDesc>(*entity);
        let _ = world.insert(
            *entity,
            DisplayVisibility {
                mode: DisplayMode::RuntimeHidden,
            },
        );
    }

    let hit_hazard = world.query::<FpsObjectiveHazard>().any(|(entity, hazard)| {
        world.get::<Transform>(entity).is_some_and(|t| {
            let r = hazard.radius.max(0.1);
            distance_sq(player_pos, t.position) <= r * r
        })
    });

    let reached_goal = world.query::<FpsObjectiveGoal>().any(|(entity, goal)| {
        world.get::<Transform>(entity).is_some_and(|t| {
            let r = goal.radius.max(0.1);
            distance_sq(player_pos, t.position) <= r * r
        })
    });

    let collected_delta = picked.len() as u32;
    let destroyed_delta = destroyed_targets.len() as u32;

    let event = if let Some(state) = world.resource_mut::<FpsObjectiveState>() {
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

    if let Some(state) = world.resource_mut::<FpsObjectiveState>() {
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
