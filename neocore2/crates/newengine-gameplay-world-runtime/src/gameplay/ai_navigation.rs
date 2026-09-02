use newengine_ecs::{EntityId, World};
use newengine_entity_api::EntityHandle;
use newengine_math::{EulerRot, Quat, Vec2, Vec3};
use newengine_navigation_api::{
    navigation_method, NavPlanPathRequestV1, NavPlanPathResponseV1, NavVec3,
    ENGINE_NAVIGATION_SERVICE_ID,
};
use newengine_sim::{CharacterFacingTurnStepRequest, CharacterMotor, MotorInput};
use newengine_transform::Transform;

use super::{
    AIController, CharacterControlState, CharacterLifeState, CombatIntent, CombatIntentKind, Health,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AINavigationTuning {
    pub move_speed: f32,
    pub investigate_arrival_distance: f32,
    pub engage_standoff_distance: f32,
    pub waypoint_arrival_distance: f32,
    pub repath_interval_seconds: f32,
    pub view_turn_speed_radians_per_second: f32,
}

impl AINavigationTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            move_speed: finite(self.move_speed, 2.5).clamp(0.0, 30.0),
            investigate_arrival_distance: finite(self.investigate_arrival_distance, 0.8)
                .clamp(0.05, 25.0),
            engage_standoff_distance: finite(self.engage_standoff_distance, 8.0).clamp(0.05, 250.0),
            waypoint_arrival_distance: finite(self.waypoint_arrival_distance, 0.35)
                .clamp(0.02, 10.0),
            repath_interval_seconds: finite(self.repath_interval_seconds, 0.35).clamp(0.05, 30.0),
            view_turn_speed_radians_per_second: finite(
                self.view_turn_speed_radians_per_second,
                240.0_f32.to_radians(),
            )
            .clamp(1.0_f32.to_radians(), 1440.0_f32.to_radians()),
        }
    }
}

impl Default for AINavigationTuning {
    fn default() -> Self {
        Self {
            move_speed: 2.5,
            investigate_arrival_distance: 0.8,
            engage_standoff_distance: 8.0,
            waypoint_arrival_distance: 0.35,
            repath_interval_seconds: 0.35,
            view_turn_speed_radians_per_second: 240.0_f32.to_radians(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AIPatrolRoute {
    pub waypoints: Vec<Vec3>,
    pub looping: bool,
}

impl AIPatrolRoute {
    pub fn new(waypoints: Vec<Vec3>) -> Self {
        Self {
            waypoints: waypoints
                .into_iter()
                .filter(|point| point.is_finite())
                .collect(),
            looping: true,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.waypoints.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AIPatrolState {
    pub waypoint_index: usize,
    pub completed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AINavigationState {
    pub goal: Option<Vec3>,
    pub path: Vec<Vec3>,
    pub waypoint_index: usize,
    pub repath_remaining_seconds: f32,
    pub revision: u64,
}

#[inline]
fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[inline]
fn nav_vec3(value: Vec3) -> NavVec3 {
    NavVec3::new(value.x, value.y, value.z)
}

#[inline]
fn vec3(value: NavVec3) -> Option<Vec3> {
    let value = Vec3::new(value.x, value.y, value.z);
    value.is_finite().then_some(value)
}

#[inline]
fn wrap_pi(angle: f32) -> f32 {
    if angle.is_finite() {
        (angle + core::f32::consts::PI).rem_euclid(core::f32::consts::TAU) - core::f32::consts::PI
    } else {
        0.0
    }
}

#[inline]
fn yaw_pitch_to(origin: Vec3, target: Vec3) -> Option<(f32, f32)> {
    let delta = target - origin;
    if !delta.is_finite() || delta.length_squared() <= 1.0e-8 {
        return None;
    }
    let horizontal = Vec3::new(delta.x, 0.0, delta.z);
    let horizontal_len = horizontal.length();
    let yaw = (-delta.x).atan2(-delta.z);
    let pitch = delta.y.atan2(horizontal_len.max(1.0e-6));
    Some((yaw, pitch))
}

#[inline]
fn operational(world: &World, entity: EntityId) -> bool {
    world
        .get::<AIController>(entity)
        .is_some_and(|controller| controller.enabled)
        && world
            .get::<CharacterControlState>(entity)
            .is_none_or(|state| state.enabled)
        && world
            .get::<CharacterLifeState>(entity)
            .is_none_or(|state| state.alive())
        && world
            .get::<Health>(entity)
            .is_none_or(|health| health.alive())
}

fn stop_motor_input(world: &mut World, entity: EntityId) {
    if world.get::<MotorInput>(entity).is_some() {
        let _ = world.insert(entity, MotorInput::default());
    }
    let _ = world.remove::<CharacterFacingTurnStepRequest>(entity);
}

fn plan_path(
    entity: EntityId,
    start: Vec3,
    goal: Vec3,
    intent: CombatIntentKind,
) -> Result<Option<Vec<Vec3>>, String> {
    let request = NavPlanPathRequestV1 {
        agent: Some(EntityHandle::new(entity.stable_u64())),
        start: nav_vec3(start),
        goal: nav_vec3(goal),
        tags: Vec::new(),
        constraints: serde_json::json!({"intent": intent.as_str()}),
    };
    let payload = serde_json::to_vec(&request)
        .map_err(|error| format!("navigation request encode failed: {error}"))?;
    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_NAVIGATION_SERVICE_ID,
        navigation_method::PLAN_PATH_JSON_V1,
        &payload,
    )?
    else {
        return Ok(None);
    };
    let response: NavPlanPathResponseV1 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("navigation response decode failed: {error}"))?;
    if !response.accepted {
        return Ok(None);
    }
    let Some(path) = response.path.filter(|path| path.complete) else {
        return Ok(None);
    };
    let points = path
        .points
        .into_iter()
        .filter_map(|point| vec3(point.position))
        .collect::<Vec<_>>();
    Ok((!points.is_empty()).then_some(points))
}

fn patrol_goal(world: &World, entity: EntityId) -> Option<Vec3> {
    let route = world.get::<AIPatrolRoute>(entity)?;
    if route.waypoints.is_empty() {
        return None;
    }
    let state = world
        .get::<AIPatrolState>(entity)
        .copied()
        .unwrap_or_default();
    if state.completed {
        return None;
    }
    route
        .waypoints
        .get(state.waypoint_index.min(route.waypoints.len() - 1))
        .copied()
}

fn goal_for_intent(world: &World, entity: EntityId, intent: CombatIntent) -> Option<Vec3> {
    match intent.kind {
        CombatIntentKind::Idle => patrol_goal(world, entity),
        CombatIntentKind::Investigate => intent
            .target_position
            .is_finite()
            .then_some(intent.target_position),
        CombatIntentKind::Engage => intent
            .target
            .and_then(|target| {
                world
                    .get::<Transform>(target)
                    .map(|transform| transform.position)
            })
            .or_else(|| {
                intent
                    .target_position
                    .is_finite()
                    .then_some(intent.target_position)
            }),
    }
}

fn target_aim_point(world: &World, intent: CombatIntent) -> Option<Vec3> {
    let target = intent.target?;
    let transform = world.get::<Transform>(target)?;
    let eye_height = world
        .get::<super::CharacterBody>(target)
        .map(|body| body.sanitized().standing_eye_height)
        .unwrap_or(0.0);
    Some(transform.position + Vec3::Y * eye_height)
}

fn apply_look_and_facing(
    world: &mut World,
    entity: EntityId,
    aim_point: Vec3,
    moving: bool,
    tuning: AINavigationTuning,
    dt: f32,
    input: &mut MotorInput,
) {
    let Some(origin_transform) = world.get::<Transform>(entity).copied() else {
        return;
    };
    let eye_height = world
        .get::<super::CharacterBody>(entity)
        .map(|body| body.sanitized().standing_eye_height)
        .unwrap_or(0.0);
    let origin = origin_transform.position + Vec3::Y * eye_height;
    let Some((desired_yaw, desired_pitch)) = yaw_pitch_to(origin, aim_point) else {
        return;
    };
    let Some(motor) = world.get::<CharacterMotor>(entity).copied() else {
        return;
    };
    let max_step = tuning.view_turn_speed_radians_per_second * dt;
    let yaw_step = wrap_pi(desired_yaw - motor.yaw).clamp(-max_step, max_step);
    let pitch_step = (desired_pitch - motor.pitch).clamp(-max_step, max_step);
    let look_sens = motor.look_sens.abs().max(1.0e-6);
    input.look_active = true;
    input.look_delta = Vec2::new(yaw_step / look_sens, pitch_step / look_sens);
    input.face_view = true;

    if !moving {
        let (body_yaw, _, _) = origin_transform
            .rotation
            .normalize_or_identity()
            .to_euler(EulerRot::YXZ);
        let body_delta = wrap_pi(desired_yaw - body_yaw);
        let max_body_step = motor.body_turn_speed.max(0.1) * dt;
        if body_delta.abs() > 1.0e-4 {
            let _ = world.insert(
                entity,
                CharacterFacingTurnStepRequest {
                    yaw_delta: body_delta.clamp(-max_body_step, max_body_step),
                },
            );
        }
    }
}

pub fn step_ai_navigation_actuation(world: &mut World, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let agents = world
        .query::<AINavigationTuning>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for entity in agents {
        if !operational(world, entity) {
            stop_motor_input(world, entity);
            let _ = world.insert(entity, AINavigationState::default());
            continue;
        }
        let tuning = world
            .get::<AINavigationTuning>(entity)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let intent = world
            .get::<CombatIntent>(entity)
            .copied()
            .unwrap_or_default();
        let Some(goal) = goal_for_intent(world, entity, intent) else {
            stop_motor_input(world, entity);
            let _ = world.insert(entity, AINavigationState::default());
            continue;
        };
        let Some(transform) = world.get::<Transform>(entity).copied() else {
            stop_motor_input(world, entity);
            continue;
        };
        let Some(motor) = world.get::<CharacterMotor>(entity).copied() else {
            stop_motor_input(world, entity);
            continue;
        };

        let mut state = world
            .get::<AINavigationState>(entity)
            .cloned()
            .unwrap_or_default();
        state.repath_remaining_seconds = (state.repath_remaining_seconds - dt).max(0.0);
        let goal_changed = state
            .goal
            .is_none_or(|old| (old - goal).length_squared() > 0.25 * 0.25);
        if goal_changed || state.path.is_empty() || state.repath_remaining_seconds <= 1.0e-6 {
            match plan_path(entity, transform.position, goal, intent.kind) {
                Ok(Some(path)) => {
                    state.path = path;
                    state.waypoint_index = usize::from(state.path.first().is_some_and(|point| {
                        (*point - transform.position).length_squared() < 0.05 * 0.05
                    }));
                    state.goal = Some(goal);
                    state.repath_remaining_seconds = tuning.repath_interval_seconds;
                    state.revision = state.revision.wrapping_add(1);
                }
                Ok(None) => {
                    state.path.clear();
                    state.waypoint_index = 0;
                    state.goal = Some(goal);
                    state.repath_remaining_seconds = tuning.repath_interval_seconds;
                }
                Err(error) => {
                    newengine_ulog_api::ulog::warn!(
                        "AI navigation plan failed entity={} err='{}'",
                        entity.stable_u64(),
                        error,
                    );
                    state.path.clear();
                    state.waypoint_index = 0;
                    state.goal = Some(goal);
                    state.repath_remaining_seconds = tuning.repath_interval_seconds;
                }
            }
        }

        let horizontal_to_goal = Vec3::new(
            goal.x - transform.position.x,
            0.0,
            goal.z - transform.position.z,
        );
        let stop_distance = match intent.kind {
            CombatIntentKind::Idle => tuning.waypoint_arrival_distance,
            CombatIntentKind::Investigate => tuning.investigate_arrival_distance,
            CombatIntentKind::Engage => tuning.engage_standoff_distance,
        };
        let arrived = horizontal_to_goal.length() <= stop_distance;

        if arrived && matches!(intent.kind, CombatIntentKind::Idle) {
            if let Some(route) = world.get::<AIPatrolRoute>(entity).cloned() {
                if !route.waypoints.is_empty() {
                    let mut patrol = world
                        .get::<AIPatrolState>(entity)
                        .copied()
                        .unwrap_or_default();
                    if patrol.waypoint_index + 1 < route.waypoints.len() {
                        patrol.waypoint_index += 1;
                    } else if route.looping {
                        patrol.waypoint_index = 0;
                    } else {
                        patrol.completed = true;
                    }
                    let _ = world.insert(entity, patrol);
                    state.path.clear();
                    state.goal = None;
                    state.waypoint_index = 0;
                    state.repath_remaining_seconds = 0.0;
                    state.revision = state.revision.wrapping_add(1);
                }
            }
        }

        while !arrived && state.waypoint_index < state.path.len() {
            let waypoint = state.path[state.waypoint_index];
            let delta = Vec3::new(
                waypoint.x - transform.position.x,
                0.0,
                waypoint.z - transform.position.z,
            );
            if delta.length() > tuning.waypoint_arrival_distance {
                break;
            }
            state.waypoint_index += 1;
        }

        let mut input = MotorInput::default();
        let mut moving = false;
        if !arrived {
            let waypoint = state
                .path
                .get(state.waypoint_index)
                .copied()
                .unwrap_or(goal);
            let direction_ws = Vec3::new(
                waypoint.x - transform.position.x,
                0.0,
                waypoint.z - transform.position.z,
            )
            .normalize_or_zero();
            if direction_ws.length_squared() > 1.0e-8 && tuning.move_speed > 0.0 {
                let local = Quat::from_rotation_y(-motor.yaw) * direction_ws;
                let forward_sign =
                    if motor.forward_sign_z.is_finite() && motor.forward_sign_z != 0.0 {
                        motor.forward_sign_z.signum()
                    } else {
                        -1.0
                    };
                input.move_axis = Vec3::new(local.x, 0.0, local.z / forward_sign);
                input.speed_mul = if motor.move_speed > 1.0e-5 {
                    tuning.move_speed / motor.move_speed
                } else {
                    1.0
                };
                moving = true;
            }
        }

        if matches!(intent.kind, CombatIntentKind::Engage) {
            if let Some(aim_point) = target_aim_point(world, intent) {
                apply_look_and_facing(world, entity, aim_point, moving, tuning, dt, &mut input);
            }
        }
        let _ = world.insert(entity, input);
        let _ = world.insert(entity, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_math::Quat;

    fn spawn_agent(world: &mut World, position: Vec3) -> EntityId {
        let entity = world.spawn();
        let _ = world.insert(
            entity,
            Transform {
                position,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        let _ = world.insert(entity, AIController::default());
        let _ = world.insert(entity, CharacterControlState::enabled());
        let _ = world.insert(entity, CharacterLifeState::Alive);
        let _ = world.insert(entity, Health::new(100.0));
        let _ = world.insert(entity, CharacterMotor::default());
        let _ = world.insert(entity, MotorInput::default());
        let _ = world.insert(entity, AINavigationTuning::default());
        entity
    }

    #[test]
    fn idle_clears_motor_actuation() {
        let mut world = World::new();
        let agent = spawn_agent(&mut world, Vec3::ZERO);
        let _ = world.insert(
            agent,
            MotorInput {
                move_axis: Vec3::Z,
                speed_mul: 1.0,
                ..MotorInput::default()
            },
        );
        step_ai_navigation_actuation(&mut world, 1.0 / 60.0);
        assert_eq!(
            world.get::<MotorInput>(agent).unwrap().move_axis,
            Vec3::ZERO
        );
    }

    #[test]
    fn idle_patrol_route_drives_character_motor_input() {
        let mut world = World::new();
        let agent = spawn_agent(&mut world, Vec3::ZERO);
        let goal = Vec3::new(0.0, 0.0, -6.0);
        let _ = world.insert(
            agent,
            AIPatrolRoute {
                waypoints: vec![goal, Vec3::new(2.0, 0.0, -6.0)],
                looping: true,
            },
        );
        let _ = world.insert(agent, AIPatrolState::default());
        let _ = world.insert(
            agent,
            AINavigationState {
                goal: Some(goal),
                path: vec![Vec3::ZERO, goal],
                waypoint_index: 1,
                repath_remaining_seconds: 1.0,
                revision: 1,
            },
        );

        step_ai_navigation_actuation(&mut world, 1.0 / 60.0);

        let input = *world.get::<MotorInput>(agent).expect("patrol motor input");
        assert!(input.move_axis.z > 0.9, "input={input:?}");
        assert_eq!(world.get::<Transform>(agent).unwrap().position, Vec3::ZERO);
    }

    #[test]
    fn injected_navigation_path_drives_character_motor_input_without_transform_write() {
        let mut world = World::new();
        let agent = spawn_agent(&mut world, Vec3::ZERO);
        let _ = world.insert(
            agent,
            CombatIntent {
                kind: CombatIntentKind::Investigate,
                target: None,
                target_position: Vec3::new(0.0, 0.0, -10.0),
                revision: 1,
            },
        );
        let _ = world.insert(
            agent,
            AINavigationState {
                goal: Some(Vec3::new(0.0, 0.0, -10.0)),
                path: vec![Vec3::ZERO, Vec3::new(0.0, 0.0, -10.0)],
                waypoint_index: 1,
                repath_remaining_seconds: 1.0,
                revision: 1,
            },
        );
        step_ai_navigation_actuation(&mut world, 1.0 / 60.0);
        let input = *world.get::<MotorInput>(agent).unwrap();
        assert!(input.move_axis.z > 0.9, "input={input:?}");
        assert_eq!(world.get::<Transform>(agent).unwrap().position, Vec3::ZERO);
    }

    #[test]
    fn engage_stationary_requests_bounded_facing_turn() {
        let mut world = World::new();
        let agent = spawn_agent(&mut world, Vec3::ZERO);
        let target = world.spawn();
        let _ = world.insert(
            target,
            Transform {
                position: Vec3::new(10.0, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        let _ = world.insert(target, super::super::CharacterBody::default());
        let _ = world.insert(
            agent,
            CombatIntent {
                kind: CombatIntentKind::Engage,
                target: Some(target),
                target_position: Vec3::new(10.0, 0.0, 0.0),
                revision: 1,
            },
        );
        let _ = world.insert(
            agent,
            AINavigationTuning {
                engage_standoff_distance: 20.0,
                ..AINavigationTuning::default()
            },
        );
        let _ = world.insert(
            agent,
            AINavigationState {
                goal: Some(Vec3::new(10.0, 0.0, 0.0)),
                path: vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)],
                waypoint_index: 1,
                repath_remaining_seconds: 1.0,
                revision: 1,
            },
        );
        step_ai_navigation_actuation(&mut world, 1.0 / 60.0);
        let turn = world
            .get::<CharacterFacingTurnStepRequest>(agent)
            .copied()
            .expect("turn request");
        assert!(turn.yaw_delta.abs() > 0.0);
        assert!(turn.yaw_delta.abs() <= CharacterMotor::default().body_turn_speed / 60.0 + 1.0e-5);
    }
}
