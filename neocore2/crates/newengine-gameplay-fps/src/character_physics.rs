#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    apply_player_stance_geometry, emit_player_event, CollisionShapeDesc, PhysicsBodyDesc,
    PhysicsSurface, PhysicsWorldSettings, PlayerController, PlayerEventKind, PlayerGroundState,
    PlayerLocomotionState, PlayerStanceKind, PlayerStanceState,
};
use newengine_gameplay_fps_api::{FpsDemoRules, FpsPlayerTuning};
use newengine_math::{Vec2, Vec3};
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto, PhysicsQueryKindDto};
use newengine_sim::{CharacterMotor, Velocity};
use newengine_transform::Transform;

const STAND_PROBE_SAMPLE_COUNT: usize = 5;
const STAND_PROBE_QUERY_SALT: u64 = 0x9e37_79b9_7f4a_7c15;

#[inline]
fn tuning(world: &World) -> FpsPlayerTuning {
    world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_else(|| FpsPlayerTuning::default().sanitized())
}

/// Projects FPS-authored gravity/contact policy into the provider-neutral physics world resource.
/// The engine physics bridge consumes only `PhysicsWorldSettings` and never reads FPS rules.
pub(crate) fn sync_physics_world_settings(world: &mut World) {
    let tuning = tuning(world);
    world.insert_resource(
        PhysicsWorldSettings {
            gravity: tuning.gravity,
            contact_skin: tuning.contact_skin,
        }
        .sanitized(),
    );
}

pub(crate) fn collect_character_queries(world: &World) -> Vec<PhysicsQueryDto> {
    let tuning = tuning(world);
    let mut queries = collect_ground_queries(world, tuning);
    queries.extend(collect_stand_clearance_queries(world, tuning));
    queries
}

pub(crate) fn resolve_character_query_hits(
    world: &mut World,
    fixed_tick: u64,
    hits: &[PhysicsQueryHitDto],
    key_to_entity: &BTreeMap<u64, EntityId>,
) -> BTreeSet<u64> {
    reset_ground_states(world, fixed_tick);
    let mut consumed = BTreeSet::new();
    let mut blocked_stand_probes = BTreeSet::new();

    for hit in hits {
        if let Some(player) = stand_probe_owner(world, hit.seq) {
            consumed.insert(hit.seq);
            if hit.entity != player.stable_u64() {
                blocked_stand_probes.insert(player);
            }
            continue;
        }

        let Some(player) = key_to_entity.get(&hit.seq).copied() else {
            continue;
        };
        if world.get::<PlayerController>(player).is_none()
            || world.get::<CharacterMotor>(player).is_none()
            || world.get::<PhysicsBodyDesc>(player).is_none()
        {
            continue;
        }

        consumed.insert(hit.seq);
        apply_ground_query_hit(world, key_to_entity, fixed_tick, *hit);
    }

    resolve_stand_clearance(world, fixed_tick, &blocked_stand_probes);
    consumed
}

/// Runs FPS-only footstep/landing policy after the physics provider has resolved ground probes.
pub(crate) fn step_character_locomotion(world: &mut World, dt: f32) {
    let key_to_entity = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    update_player_locomotion(world, &key_to_entity, dt);
}

fn collect_ground_queries(world: &World, tuning: FpsPlayerTuning) -> Vec<PhysicsQueryDto> {
    let tuning = tuning.sanitized();
    let epsilon = ground_probe_origin_epsilon(tuning.contact_skin);
    let max_t = (tuning.contact_skin + tuning.ground_probe_distance).max(0.01);
    let mut queries = Vec::new();

    for entity in world.query2_ids::<CharacterMotor, PhysicsBodyDesc>() {
        if world.get::<PlayerController>(entity).is_none() {
            continue;
        }
        let Some(transform) = world.get::<Transform>(entity).copied() else {
            continue;
        };
        let Some(body) = world.get::<PhysicsBodyDesc>(entity).copied() else {
            continue;
        };
        if !body.flags.participates_in_queries {
            continue;
        }
        let vertical_extent = collision_shape_vertical_extent(body.shape);
        queries.push(PhysicsQueryDto {
            seq: entity.stable_u64(),
            kind: PhysicsQueryKindDto::Ray {
                origin: [
                    transform.position.x,
                    transform.position.y - vertical_extent - epsilon,
                    transform.position.z,
                ],
                dir: [0.0, -1.0, 0.0],
                max_t,
            },
        });
    }
    queries
}

#[inline]
fn stand_probe_query_seq(player_key: u64, sample_index: usize) -> u64 {
    player_key.rotate_left(23)
        ^ STAND_PROBE_QUERY_SALT
        ^ (sample_index as u64).wrapping_mul(0xd6e8_feb8_6659_fd93)
}

fn stand_probe_owner(world: &World, query_seq: u64) -> Option<EntityId> {
    for (player, stance) in world.query::<PlayerStanceState>() {
        if stance.current != PlayerStanceKind::Crouched || !stance.stand_requested {
            continue;
        }
        for sample_index in 0..STAND_PROBE_SAMPLE_COUNT {
            if stand_probe_query_seq(player.stable_u64(), sample_index) == query_seq {
                return Some(player);
            }
        }
    }
    None
}

fn collect_stand_clearance_queries(world: &World, tuning: FpsPlayerTuning) -> Vec<PhysicsQueryDto> {
    let tuning = tuning.sanitized();
    let epsilon = ground_probe_origin_epsilon(tuning.contact_skin);
    let mut queries = Vec::new();

    for (player, stance) in world.query::<PlayerStanceState>() {
        if stance.current != PlayerStanceKind::Crouched || !stance.stand_requested {
            continue;
        }
        let Some(transform) = world.get::<Transform>(player).copied() else {
            continue;
        };
        let Some(body) = world.get::<PhysicsBodyDesc>(player).copied() else {
            continue;
        };
        let CollisionShapeDesc::Capsule {
            radius,
            half_height: current_half_height,
        } = body.shape.sanitized()
        else {
            continue;
        };
        let half_height_delta = (tuning.body_half_height - current_half_height).max(0.0);
        if half_height_delta <= 1.0e-5 {
            continue;
        }

        let max_t = (2.0 * half_height_delta + tuning.contact_skin).max(0.01);
        let top_y = transform.position.y + current_half_height + radius + epsilon;
        let radial = (radius * 0.62).max(0.01);
        let offsets = [
            [0.0, 0.0],
            [radial, 0.0],
            [-radial, 0.0],
            [0.0, radial],
            [0.0, -radial],
        ];
        for (sample_index, [offset_x, offset_z]) in offsets.into_iter().enumerate() {
            queries.push(PhysicsQueryDto {
                seq: stand_probe_query_seq(player.stable_u64(), sample_index),
                kind: PhysicsQueryKindDto::Ray {
                    origin: [
                        transform.position.x + offset_x,
                        top_y,
                        transform.position.z + offset_z,
                    ],
                    dir: [0.0, 1.0, 0.0],
                    max_t,
                },
            });
        }
    }
    queries
}

#[inline]
fn ground_probe_origin_epsilon(contact_skin: f32) -> f32 {
    (contact_skin.abs() * 0.25).clamp(0.001, 0.01)
}

#[inline]
fn collision_shape_vertical_extent(shape: CollisionShapeDesc) -> f32 {
    match shape.sanitized() {
        CollisionShapeDesc::Box { half_extents } => half_extents[1],
        CollisionShapeDesc::Sphere { radius } => radius,
        CollisionShapeDesc::Capsule {
            radius,
            half_height,
        } => radius + half_height,
    }
}

fn reset_ground_states(world: &mut World, fixed_tick: u64) {
    let players = world
        .query2_ids::<PlayerController, PhysicsBodyDesc>()
        .collect::<Vec<_>>();
    for player in players {
        if world.get::<CharacterMotor>(player).is_none() {
            continue;
        }
        if world.get::<PlayerGroundState>(player).is_none() {
            let _ = world.insert(player, PlayerGroundState::default());
        }
        if let Some(state) = world.get_mut::<PlayerGroundState>(player) {
            state.clear_for_tick(fixed_tick);
        }
    }
}

fn apply_ground_query_hit(
    world: &mut World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    fixed_tick: u64,
    hit: PhysicsQueryHitDto,
) {
    let Some(player) = key_to_entity.get(&hit.seq).copied() else {
        return;
    };
    if hit.entity == hit.seq || world.get::<CharacterMotor>(player).is_none() {
        return;
    }
    let tuning = tuning(world);
    let max_distance = tuning.contact_skin + tuning.ground_probe_distance;
    if !hit.distance.is_finite() || !(0.0..=max_distance).contains(&hit.distance) {
        return;
    }
    let vertical_velocity = world
        .get::<Velocity>(player)
        .map(|velocity| velocity.0.y)
        .unwrap_or(0.0);
    if !vertical_velocity.is_finite() || vertical_velocity > 0.1 {
        return;
    }

    let mut normal = Vec3::new(hit.normal[0], hit.normal[1], hit.normal[2]);
    if !normal.is_finite() || normal.length_squared() <= 1.0e-8 {
        normal = Vec3::Y;
    } else {
        normal = normal.normalize();
    }
    let slope_radians = normal.y.clamp(-1.0, 1.0).acos();
    let walkable = slope_radians <= tuning.max_slope_radians;

    if let Some(state) = world.get_mut::<PlayerGroundState>(player) {
        state.grounded = walkable;
        state.walkable = walkable;
        state.ground_entity = Some(hit.entity);
        state.distance = hit.distance.max(0.0);
        state.normal = normal;
        state.slope_radians = slope_radians;
        state.last_fixed_tick = fixed_tick;
    }
}

fn resolve_stand_clearance(world: &mut World, fixed_tick: u64, blocked: &BTreeSet<EntityId>) {
    let requested = world
        .query::<PlayerStanceState>()
        .filter_map(|(player, state)| {
            (state.current == PlayerStanceKind::Crouched && state.stand_requested).then_some(player)
        })
        .collect::<Vec<_>>();

    for player in requested {
        if blocked.contains(&player) {
            let should_emit = world
                .get::<PlayerStanceState>(player)
                .map(|state| !state.stand_blocked)
                .unwrap_or(false);
            if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
                state.stand_blocked = true;
            }
            if should_emit {
                emit_player_event(
                    world,
                    player,
                    PlayerEventKind::StanceBlocked,
                    "stand blocked by overhead collision",
                );
            }
        } else {
            let _ =
                apply_player_stance_geometry(world, player, PlayerStanceKind::Standing, fixed_tick);
        }
    }
}

fn update_player_locomotion(world: &mut World, key_to_entity: &BTreeMap<u64, EntityId>, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let tuning = tuning(world);
    let players = world
        .query2_ids::<CharacterMotor, PlayerGroundState>()
        .filter(|player| world.get::<PlayerController>(*player).is_some())
        .collect::<Vec<_>>();

    for player in players {
        let ground = world
            .get::<PlayerGroundState>(player)
            .copied()
            .unwrap_or_default();
        let velocity = world.get::<Velocity>(player).copied().unwrap_or_default().0;
        let horizontal_speed = Vec2::new(velocity.x, velocity.z).length();
        let surface = ground
            .ground_entity
            .and_then(|key| key_to_entity.get(&key).copied())
            .and_then(|entity| world.get::<PhysicsSurface>(entity).cloned())
            .unwrap_or_default();

        if world.get::<PlayerLocomotionState>(player).is_none() {
            let _ = world.insert(player, PlayerLocomotionState::default());
        }

        let mut emitted = Vec::<(PlayerEventKind, String)>::new();
        if let Some(state) = world.get_mut::<PlayerLocomotionState>(player) {
            if ground.grounded {
                if !state.was_grounded {
                    emitted.push((
                        PlayerEventKind::GroundStateChanged,
                        format!(
                            "grounded surface='{}' slope_deg={:.1}",
                            surface.id,
                            ground.slope_radians.to_degrees()
                        ),
                    ));
                    if state.airborne_time > 0.05
                        && state.max_downward_speed >= tuning.landing_speed_threshold
                    {
                        emitted.push((
                            PlayerEventKind::Landed,
                            format!(
                                "{} surface='{}' speed={:.2}",
                                surface.landing_event, surface.id, state.max_downward_speed
                            ),
                        ));
                    }
                }

                if dt > 0.0 && horizontal_speed > 0.15 {
                    state.step_distance += horizontal_speed * dt;
                    if state.step_distance >= tuning.footstep_stride {
                        state.step_distance =
                            (state.step_distance - tuning.footstep_stride).max(0.0);
                        emitted.push((
                            PlayerEventKind::Footstep,
                            format!(
                                "{} surface='{}' speed={:.2}",
                                surface.footstep_event, surface.id, horizontal_speed
                            ),
                        ));
                    }
                }
                state.airborne_time = 0.0;
                state.max_downward_speed = 0.0;
            } else {
                if state.was_grounded {
                    emitted.push((PlayerEventKind::GroundStateChanged, "airborne".to_owned()));
                }
                state.step_distance = 0.0;
                state.airborne_time += dt;
                if velocity.y.is_finite() {
                    state.max_downward_speed = state.max_downward_speed.max((-velocity.y).max(0.0));
                }
            }
            state.was_grounded = ground.grounded;
        }

        for (kind, message) in emitted {
            emit_player_event(world, player, kind, message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::spawn_default_player;

    #[test]
    fn ground_probe_is_owned_by_fps_provider() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning::default().sanitized();
        let vertical_extent = tuning.body_half_height + tuning.body_radius;
        let player = spawn_default_player(
            &mut world,
            None,
            "fps-ground-probe-player",
            Vec3::new(3.0, vertical_extent + tuning.contact_skin, -2.0),
        );
        let queries = collect_character_queries(&world);
        assert!(queries.iter().any(|query| query.seq == player.stable_u64()));
    }
}
