use super::*;

pub(super) fn reset_ground_states(world: &mut World, fixed_tick: u64) {
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

pub(super) fn apply_ground_query_hit(
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

pub(super) fn resolve_stand_clearance(
    world: &mut World,
    fixed_tick: u64,
    blocked: &BTreeSet<EntityId>,
) {
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

pub(super) fn update_player_locomotion(
    world: &mut World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    dt: f32,
) {
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
