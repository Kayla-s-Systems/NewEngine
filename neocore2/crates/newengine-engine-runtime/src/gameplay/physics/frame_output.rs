use std::collections::{BTreeMap, BTreeSet};

use newengine_ecs::{EntityId, World};
use newengine_physics_api::{
    PhysicsBodyPoseUpdate, PhysicsBodyVelocityUpdate, PhysicsFrameOutput, PhysicsQueryHitDto,
    PhysicsStepReportDto,
};
use newengine_physics_contracts::{
    PhysicsBodyDesc, PhysicsContactEvent, PhysicsEvent, PhysicsStepReport,
};
use newengine_sim::{CharacterMotor, Velocity};
use newengine_transform::Transform;

use crate::gameplay::{
    apply_player_stance_geometry, emit_player_event, resolve_combat_queries, FpsDemoRules,
    FpsPlayerTuning, PhysicsSurface, PlayerEventKind, PlayerGroundState, PlayerLocomotionState,
    PlayerStanceKind, PlayerStanceState,
};

use super::frame_input::stand_probe_owner;
use super::util::{arr_to_quat, arr_to_vec3};

pub(super) fn apply_frame_output(world: &mut World, output: PhysicsFrameOutput) {
    let PhysicsFrameOutput {
        fixed_tick,
        pose_updates,
        velocity_updates,
        events,
        query_hits,
        report,
    } = output;
    // Query/contact outputs may reference service-owned terrain colliders that do not carry a
    // native PhysicsBodyDesc component. Resolve against the complete live entity table so surface,
    // damage and contact events are not silently dropped at the host boundary.
    let key_to_entity = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();

    for update in pose_updates {
        apply_pose_update(world, &key_to_entity, update);
    }

    for update in velocity_updates {
        apply_velocity_update(world, &key_to_entity, update);
    }

    reset_ground_states(world, fixed_tick);
    let consumed_combat_queries =
        resolve_combat_queries(world, fixed_tick, &query_hits, &key_to_entity);
    let mut blocked_stand_probes = BTreeSet::new();
    for hit in query_hits {
        if consumed_combat_queries.contains(&hit.seq) {
            continue;
        }
        if let Some(player) = stand_probe_owner(world, hit.seq) {
            if hit.entity != player.stable_u64() {
                blocked_stand_probes.insert(player);
            }
            continue;
        }
        apply_ground_query_hit(world, &key_to_entity, fixed_tick, hit);
    }
    resolve_stand_clearance(world, fixed_tick, &blocked_stand_probes);
    update_player_locomotion(world, &key_to_entity, report.dt);

    world.insert_resource(report_from_dto(report, events, &key_to_entity));
}

fn reset_ground_states(world: &mut World, fixed_tick: u64) {
    let players = world
        .query2_ids::<CharacterMotor, PhysicsBodyDesc>()
        .collect::<Vec<_>>();
    for player in players {
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
    let tuning = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_else(|| FpsPlayerTuning::default().sanitized());
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

    let mut normal = arr_to_vec3(hit.normal);
    if !normal.is_finite() || normal.length_squared() <= 1.0e-8 {
        normal = newengine_math::Vec3::Y;
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
    let tuning = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_else(|| FpsPlayerTuning::default().sanitized());
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
            let _ = apply_player_stance_geometry(
                world,
                player,
                PlayerStanceKind::Standing,
                tuning,
                fixed_tick,
            );
        }
    }
}

fn update_player_locomotion(world: &mut World, key_to_entity: &BTreeMap<u64, EntityId>, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let tuning = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_else(|| FpsPlayerTuning::default().sanitized());
    let players = world
        .query2_ids::<CharacterMotor, PlayerGroundState>()
        .collect::<Vec<_>>();

    for player in players {
        let ground = world
            .get::<PlayerGroundState>(player)
            .copied()
            .unwrap_or_default();
        let velocity = world.get::<Velocity>(player).copied().unwrap_or_default().0;
        let horizontal_speed = newengine_math::Vec2::new(velocity.x, velocity.z).length();
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

fn apply_pose_update(
    world: &mut World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    update: PhysicsBodyPoseUpdate,
) {
    let Some(entity) = key_to_entity.get(&update.entity).copied() else {
        return;
    };
    let controlled_body = is_directly_controlled_body(world, entity);
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position = arr_to_vec3(update.position);
        if !controlled_body {
            transform.rotation = arr_to_quat(update.rotation);
        }
    }
}

fn apply_velocity_update(
    world: &mut World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    update: PhysicsBodyVelocityUpdate,
) {
    let Some(entity) = key_to_entity.get(&update.entity).copied() else {
        return;
    };
    let physics_velocity = arr_to_vec3(update.linear_velocity);
    let next = if is_directly_controlled_body(world, entity) {
        let current = world.get::<Velocity>(entity).copied().unwrap_or_default().0;
        // Character motor owns lateral velocity and look/yaw. Physics owns vertical
        // resolution/gravity. This prevents the backend from erasing WASD and
        // mouse look while still applying floor contacts.
        newengine_math::Vec3::new(current.x, physics_velocity.y, current.z)
    } else {
        physics_velocity
    };
    let _ = world.insert(entity, Velocity(next));
}

#[inline]
fn is_directly_controlled_body(world: &World, entity: EntityId) -> bool {
    world.get::<CharacterMotor>(entity).is_some()
}

fn contact_from_dto(
    contact: newengine_physics_api::PhysicsContactEventDto,
    key_to_entity: &BTreeMap<u64, EntityId>,
) -> Option<PhysicsContactEvent> {
    let a = key_to_entity.get(&contact.a).copied()?;
    let b = key_to_entity.get(&contact.b).copied()?;
    Some(PhysicsContactEvent {
        a: a.into(),
        b: b.into(),
        point: arr_to_vec3(contact.point),
        normal: arr_to_vec3(contact.normal),
        impulse: if contact.impulse.is_finite() {
            contact.impulse.max(0.0)
        } else {
            0.0
        },
    })
}

fn report_from_dto(
    report: PhysicsStepReportDto,
    events: Vec<newengine_physics_api::PhysicsEventDto>,
    key_to_entity: &BTreeMap<u64, EntityId>,
) -> PhysicsStepReport {
    let mut converted_events = Vec::new();
    for event in events {
        match event {
            newengine_physics_api::PhysicsEventDto::ContactBegin(contact) => {
                if let Some(contact) = contact_from_dto(contact, key_to_entity) {
                    converted_events.push(PhysicsEvent::ContactBegin(contact));
                }
            }
            newengine_physics_api::PhysicsEventDto::ContactPersist(contact) => {
                if let Some(contact) = contact_from_dto(contact, key_to_entity) {
                    converted_events.push(PhysicsEvent::ContactPersist(contact));
                }
            }
            newengine_physics_api::PhysicsEventDto::ContactEnd { a, b } => {
                if let (Some(a), Some(b)) = (
                    key_to_entity.get(&a).copied(),
                    key_to_entity.get(&b).copied(),
                ) {
                    converted_events.push(PhysicsEvent::ContactEnd {
                        a: a.into(),
                        b: b.into(),
                    });
                }
            }
            newengine_physics_api::PhysicsEventDto::BodyCreated { entity } => {
                if let Some(entity) = key_to_entity.get(&entity).copied() {
                    converted_events.push(PhysicsEvent::BodyCreated {
                        entity: entity.into(),
                    });
                }
            }
            newengine_physics_api::PhysicsEventDto::BodyDestroyed { entity } => {
                if let Some(entity) = key_to_entity.get(&entity).copied() {
                    converted_events.push(PhysicsEvent::BodyDestroyed {
                        entity: entity.into(),
                    });
                }
            }
        }
    }

    PhysicsStepReport {
        fixed_tick: report.fixed_tick,
        dt: report.dt,
        substeps: report.substeps,
        active_bodies: report.active_bodies,
        static_bodies: report.static_bodies,
        dynamic_bodies: report.dynamic_bodies,
        contacts: report.contacts,
        commands_applied: report.commands_applied,
        events: converted_events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::spawn_default_player;
    use newengine_math::Vec3;
    use newengine_physics_api::{PhysicsContactEventDto, PhysicsEventDto, PhysicsQueryHitDto};

    #[test]
    fn report_conversion_preserves_contact_lifecycle() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let key_to_entity = BTreeMap::from([(a.stable_u64(), a), (b.stable_u64(), b)]);
        let contact = PhysicsContactEventDto {
            a: a.stable_u64(),
            b: b.stable_u64(),
            point: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            impulse: 4.5,
        };

        let report = report_from_dto(
            PhysicsStepReportDto {
                fixed_tick: 7,
                dt: 1.0 / 60.0,
                contacts: 1,
                ..PhysicsStepReportDto::default()
            },
            vec![
                PhysicsEventDto::ContactBegin(contact),
                PhysicsEventDto::ContactPersist(contact),
                PhysicsEventDto::ContactEnd {
                    a: a.stable_u64(),
                    b: b.stable_u64(),
                },
            ],
            &key_to_entity,
        );

        assert_eq!(report.fixed_tick, 7);
        assert_eq!(report.events.len(), 3);
        match report.events[0] {
            PhysicsEvent::ContactBegin(event) => {
                assert_eq!(event.a, a.into());
                assert_eq!(event.b, b.into());
                assert_eq!(event.point, newengine_math::Vec3::new(1.0, 2.0, 3.0));
                assert_eq!(event.normal, newengine_math::Vec3::Y);
                assert_eq!(event.impulse, 4.5);
            }
            ref other => panic!("expected contact begin, got {other:?}"),
        }
        assert!(matches!(report.events[1], PhysicsEvent::ContactPersist(_)));
        assert!(matches!(report.events[2], PhysicsEvent::ContactEnd { .. }));
    }

    #[test]
    fn report_conversion_drops_contacts_with_unknown_entities_and_sanitizes_impulse() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let key_to_entity = BTreeMap::from([(a.stable_u64(), a), (b.stable_u64(), b)]);

        let report = report_from_dto(
            PhysicsStepReportDto::default(),
            vec![
                PhysicsEventDto::ContactBegin(PhysicsContactEventDto {
                    a: a.stable_u64(),
                    b: b.stable_u64(),
                    point: [0.0; 3],
                    normal: [0.0, 1.0, 0.0],
                    impulse: f32::NAN,
                }),
                PhysicsEventDto::ContactPersist(PhysicsContactEventDto {
                    a: a.stable_u64(),
                    b: u64::MAX,
                    point: [0.0; 3],
                    normal: [0.0, 1.0, 0.0],
                    impulse: 1.0,
                }),
            ],
            &key_to_entity,
        );

        assert_eq!(report.events.len(), 1);
        match report.events[0] {
            PhysicsEvent::ContactBegin(event) => assert_eq!(event.impulse, 0.0),
            ref other => panic!("expected sanitized contact begin, got {other:?}"),
        }
    }

    #[test]
    fn ground_query_hit_marks_player_grounded_and_missing_hit_clears_it() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "grounded-player", Vec3::ZERO);
        let ground_key = u64::MAX - 7;

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 11,
                velocity_updates: vec![PhysicsBodyVelocityUpdate {
                    entity: player.stable_u64(),
                    linear_velocity: [0.0, 0.0, 0.0],
                }],
                query_hits: vec![PhysicsQueryHitDto {
                    seq: player.stable_u64(),
                    entity: ground_key,
                    position: [0.0, -0.03, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    distance: 0.03,
                }],
                ..PhysicsFrameOutput::default()
            },
        );

        let grounded = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground state");
        assert!(grounded.grounded);
        assert_eq!(grounded.ground_entity, Some(ground_key));
        assert_eq!(grounded.last_fixed_tick, 11);

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 12,
                ..PhysicsFrameOutput::default()
            },
        );
        let cleared = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground state");
        assert!(!cleared.grounded);
        assert_eq!(cleared.ground_entity, None);
        assert_eq!(cleared.last_fixed_tick, 12);
    }

    #[test]
    fn landing_and_footstep_events_use_ground_surface_profile() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning {
            footstep_stride: 0.2,
            landing_speed_threshold: 2.0,
            ..FpsPlayerTuning::default()
        }
        .sanitized();
        world.insert_resource(FpsDemoRules {
            player: tuning,
            ..FpsDemoRules::default()
        });
        let player = spawn_default_player(&mut world, None, "surface-player", Vec3::ZERO);
        let ground = world.spawn();
        crate::gameplay::ensure_physics_body(
            &mut world,
            ground,
            PhysicsBodyDesc::static_solid(newengine_physics_contracts::CollisionShapeDesc::Box {
                half_extents: [5.0, 0.5, 5.0],
            }),
        );
        let _ = world.insert(
            ground,
            PhysicsSurface {
                id: "surface.metal".to_owned(),
                footstep_event: "audio.footstep.metal".to_owned(),
                landing_event: "audio.landing.metal".to_owned(),
            },
        );
        let _ = world.insert(player, Velocity(Vec3::new(6.0, -4.0, 0.0)));
        if let Some(locomotion) = world.get_mut::<PlayerLocomotionState>(player) {
            locomotion.was_grounded = false;
            locomotion.airborne_time = 0.5;
            locomotion.max_downward_speed = 4.0;
            locomotion.step_distance = 0.15;
        }

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 30,
                velocity_updates: vec![PhysicsBodyVelocityUpdate {
                    entity: player.stable_u64(),
                    linear_velocity: [0.0, 0.0, 0.0],
                }],
                query_hits: vec![PhysicsQueryHitDto {
                    seq: player.stable_u64(),
                    entity: ground.stable_u64(),
                    position: [0.0, -0.02, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    distance: 0.02,
                }],
                report: PhysicsStepReportDto {
                    fixed_tick: 30,
                    dt: 1.0 / 60.0,
                    ..PhysicsStepReportDto::default()
                },
                ..PhysicsFrameOutput::default()
            },
        );

        let events = crate::gameplay::drain_player_events(&mut world);
        assert!(events.iter().any(|event| {
            event.kind == PlayerEventKind::Landed && event.message.contains("audio.landing.metal")
        }));
        assert!(events.iter().any(|event| {
            event.kind == PlayerEventKind::Footstep
                && event.message.contains("audio.footstep.metal")
        }));
    }

    #[test]
    fn ground_probe_rejects_surface_above_slope_limit() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "slope-player", Vec3::ZERO);
        let steep_normal = [0.866_025_4, 0.5, 0.0];

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 15,
                velocity_updates: vec![PhysicsBodyVelocityUpdate {
                    entity: player.stable_u64(),
                    linear_velocity: [0.0, 0.0, 0.0],
                }],
                query_hits: vec![PhysicsQueryHitDto {
                    seq: player.stable_u64(),
                    entity: u64::MAX - 8,
                    position: [0.0, -0.03, 0.0],
                    normal: steep_normal,
                    distance: 0.03,
                }],
                report: PhysicsStepReportDto {
                    dt: 1.0 / 60.0,
                    ..PhysicsStepReportDto::default()
                },
                ..PhysicsFrameOutput::default()
            },
        );

        let state = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground state");
        assert!(!state.grounded);
        assert!(!state.walkable);
        assert!((state.slope_radians.to_degrees() - 60.0).abs() < 0.1);
    }

    #[test]
    fn stand_clearance_hit_keeps_player_crouched() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning::default().sanitized();
        let player = spawn_default_player(&mut world, None, "blocked-stance", Vec3::ZERO);
        crate::gameplay::apply_player_stance_geometry(
            &mut world,
            player,
            PlayerStanceKind::Crouched,
            tuning,
            1,
        );
        if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
            state.stand_requested = true;
        }
        let ceiling = world.spawn();
        crate::gameplay::ensure_physics_body(
            &mut world,
            ceiling,
            PhysicsBodyDesc::static_solid(newengine_physics_contracts::CollisionShapeDesc::Box {
                half_extents: [1.0, 0.1, 1.0],
            }),
        );

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 2,
                query_hits: vec![PhysicsQueryHitDto {
                    seq: super::super::frame_input::stand_probe_query_seq(player.stable_u64(), 0),
                    entity: ceiling.stable_u64(),
                    position: [0.0, 1.0, 0.0],
                    normal: [0.0, -1.0, 0.0],
                    distance: 0.1,
                }],
                report: PhysicsStepReportDto {
                    dt: 1.0 / 60.0,
                    ..PhysicsStepReportDto::default()
                },
                ..PhysicsFrameOutput::default()
            },
        );

        let state = world
            .get::<PlayerStanceState>(player)
            .expect("stance state");
        assert_eq!(state.current, PlayerStanceKind::Crouched);
        assert!(state.stand_blocked);
    }

    #[test]
    fn clear_stand_probe_restores_standing_capsule_and_foot_plane() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning::default().sanitized();
        let initial_y = tuning.body_half_height + tuning.body_radius;
        let player = spawn_default_player(
            &mut world,
            None,
            "clear-stance",
            Vec3::new(0.0, initial_y, 0.0),
        );
        crate::gameplay::apply_player_stance_geometry(
            &mut world,
            player,
            PlayerStanceKind::Crouched,
            tuning,
            1,
        );
        if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
            state.stand_requested = true;
        }

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 2,
                report: PhysicsStepReportDto {
                    dt: 1.0 / 60.0,
                    ..PhysicsStepReportDto::default()
                },
                ..PhysicsFrameOutput::default()
            },
        );

        let state = world
            .get::<PlayerStanceState>(player)
            .expect("stance state");
        assert_eq!(state.current, PlayerStanceKind::Standing);
        let transform = world.get::<Transform>(player).expect("player transform");
        assert!((transform.position.y - initial_y).abs() < 1.0e-6);
        let body = world.get::<PhysicsBodyDesc>(player).expect("player body");
        assert!(matches!(
            body.shape,
            newengine_physics_contracts::CollisionShapeDesc::Capsule { half_height, .. }
                if (half_height - tuning.body_half_height).abs() < 1.0e-6
        ));
    }

    #[test]
    fn upward_velocity_prevents_ground_probe_from_rearming_jump() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "jumping-player", Vec3::ZERO);

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 20,
                velocity_updates: vec![PhysicsBodyVelocityUpdate {
                    entity: player.stable_u64(),
                    linear_velocity: [0.0, 5.5, 0.0],
                }],
                query_hits: vec![PhysicsQueryHitDto {
                    seq: player.stable_u64(),
                    entity: u64::MAX - 9,
                    position: [0.0, -0.02, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    distance: 0.02,
                }],
                ..PhysicsFrameOutput::default()
            },
        );

        assert!(
            !world
                .get::<PlayerGroundState>(player)
                .expect("ground state")
                .grounded
        );
    }
}
