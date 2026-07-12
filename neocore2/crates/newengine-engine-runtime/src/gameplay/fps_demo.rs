use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_transform::Transform;

use super::player::first_player;
use super::{
    DisplayMode, DisplayVisibility, FpsDemoGoal, FpsDemoHazard, FpsDemoPickup, FpsDemoRules,
    FpsDemoState, FpsDemoTarget, Health, PhysicsBodyDesc,
};

#[inline]
fn distance_sq(a: Vec3, b: Vec3) -> f32 {
    let d = a - b;
    d.length_squared()
}

pub fn step_fps_demo_gameplay(world: &mut World, dt: f32) {
    if world.resource::<FpsDemoState>().is_none() {
        return;
    }

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

    let mut hit_hazard = false;
    for (entity, hazard) in world.query::<FpsDemoHazard>() {
        let Some(t) = world.get::<Transform>(entity) else {
            continue;
        };
        let r = hazard.radius.max(0.1);
        if distance_sq(player_pos, t.position) <= r * r {
            hit_hazard = true;
            break;
        }
    }

    let mut reached_goal = false;
    for (entity, goal) in world.query::<FpsDemoGoal>() {
        let Some(t) = world.get::<Transform>(entity) else {
            continue;
        };
        let r = goal.radius.max(0.1);
        if distance_sq(player_pos, t.position) <= r * r {
            reached_goal = true;
            break;
        }
    }

    let rules = world
        .resource::<FpsDemoRules>()
        .cloned()
        .unwrap_or_default();
    let collected_delta = picked.len() as u32;
    let destroyed_delta = destroyed_targets.len() as u32;
    if let Some(state) = world.resource_mut::<FpsDemoState>() {
        state.pickups_collected = state
            .pickups_collected
            .saturating_add(collected_delta)
            .min(state.pickups_total);
        state.targets_destroyed = state
            .targets_destroyed
            .saturating_add(destroyed_delta)
            .min(state.targets_total);
        let objectives_complete = state.pickups_collected >= state.pickups_total
            && state.targets_destroyed >= state.targets_total;

        if hit_hazard {
            state.failed = true;
            state.status = rules.hazard_status.clone();
        } else if reached_goal && objectives_complete {
            state.completed = true;
            state.status = rules.goal_complete_status.clone();
        } else if reached_goal {
            state.status = rules.goal_locked_status.clone();
        } else if destroyed_delta > 0 {
            state.status = rules.target_status.clone();
        } else if collected_delta > 0 {
            state.status = rules.pickup_status.clone();
        } else {
            state.status = rules.default_status.clone();
        }
    }
}
