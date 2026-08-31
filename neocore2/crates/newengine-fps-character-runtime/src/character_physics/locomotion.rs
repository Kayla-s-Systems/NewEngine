use std::collections::BTreeMap;

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    emit_animation_pulse, emit_animation_state, emit_gameplay_event, emit_player_event,
    player_fall_is_confirmed, PhysicsSurface,
    PlayerController, PlayerEventKind, PlayerFallState, PlayerGroundState, PlayerLandingState,
    PlayerLocomotionState, PlayerMovementSpeeds, StaticMeshCollider,
};
use newengine_math::Vec2;
use newengine_sim::{CharacterMotor, Velocity};
use newengine_transform::Transform;

use super::footsteps::{
    classify_player_footstep_mode, contact_slip_ratio, contact_stride, is_sharp_direction_change,
    landing_normal_impact_speed, landing_position, phase_foot_position, surface_friction,
    update_model_foot_contacts, FootSide, FootstepLocomotionMode, FootstepPhase,
    FootstepRuntimeState,
};
use super::tuning::tuning;

fn publish_authored_surface_event(
    world: &mut World,
    player: EntityId,
    surface: &PhysicsSurface,
    signal: &str,
    payload: serde_json::Value,
) {
    let Some(event_id) = surface.event_for(signal) else {
        return;
    };
    if let Err(error) = emit_gameplay_event(world, event_id.to_owned(), Some(player), payload) {
        newengine_ulog_api::ulog::warn!(
            "locomotion project event publish rejected event='{}' player={} err='{}'",
            event_id,
            player.stable_u64(),
            error,
        );
    }
}

/// Runs FPS-owned contact/landing detection after the physics provider has resolved ground probes.
/// This layer publishes project-authored semantic events with physical payload only. It never
/// chooses an audio dictionary, cue, VFX asset, script filename, or project-specific event id.
pub fn step_character_locomotion(world: &mut World, dt: f32) {
    let mut key_to_entity = world
        .query::<PhysicsSurface>()
        .map(|(entity, _)| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    // A collider may intentionally rely on the default semantic surface. Preserve its stable
    // entity identity anyway so real collider friction participates in per-foot contact physics.
    for (entity, _) in world.query::<StaticMeshCollider>() {
        key_to_entity.entry(entity.stable_u64()).or_insert(entity);
    }
    update_player_locomotion(world, &key_to_entity, dt);
}

fn resolve_contact_surface(
    world: &World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    fallback: &PhysicsSurface,
    surface_key: Option<u64>,
) -> (Option<EntityId>, PhysicsSurface) {
    let entity = surface_key.and_then(|key| key_to_entity.get(&key).copied());
    let semantic = entity
        .and_then(|entity| world.get::<PhysicsSurface>(entity).cloned())
        .unwrap_or_else(|| {
            if surface_key.is_none() {
                fallback.clone()
            } else {
                PhysicsSurface::default()
            }
        });
    (entity, semantic)
}

fn update_player_locomotion(world: &mut World, key_to_entity: &BTreeMap<u64, EntityId>, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let tuning = tuning(world);
    let min_horizontal_speed = tuning.locomotion_min_horizontal_speed;
    let landing_min_airborne_seconds = tuning.landing_min_airborne_seconds;
    let players = world
        .query2_ids::<CharacterMotor, PlayerGroundState>()
        .filter(|player| world.get::<PlayerController>(*player).is_some())
        .filter(|player| !crate::noclip::fps_noclip_enabled(world, *player))
        .collect::<Vec<_>>();

    for player in players {
        let ground = world
            .get::<PlayerGroundState>(player)
            .copied()
            .unwrap_or_default();
        let velocity = world.get::<Velocity>(player).copied().unwrap_or_default().0;
        let current_height = world
            .get::<Transform>(player)
            .map(|transform| transform.position.y)
            .filter(|height| height.is_finite())
            .unwrap_or(0.0);
        let horizontal_velocity = Vec2::new(velocity.x, velocity.z);
        let horizontal_speed = horizontal_velocity.length();
        let travel_direction = horizontal_velocity.normalize_or_zero();
        let ground_entity = ground
            .ground_entity
            .and_then(|key| key_to_entity.get(&key).copied());
        let surface = ground_entity
            .and_then(|entity| world.get::<PhysicsSurface>(entity).cloned())
            .unwrap_or_default();
        let friction = surface_friction(world, ground_entity);
        let movement = world
            .get::<PlayerMovementSpeeds>(player)
            .copied()
            .unwrap_or_default();
        let mode = classify_player_footstep_mode(world, player, horizontal_speed);

        let mut locomotion = world
            .get::<PlayerLocomotionState>(player)
            .copied()
            .unwrap_or_default();
        let mut fall = world
            .get::<PlayerFallState>(player)
            .copied()
            .unwrap_or(PlayerFallState {
                start_height: current_height,
                peak_height: current_height,
                current_height,
                ..PlayerFallState::default()
            });
        let mut landing = world
            .get::<PlayerLandingState>(player)
            .copied()
            .unwrap_or_default();
        let landing_revision_before = landing.revision;
        let mut footsteps = world
            .get::<FootstepRuntimeState>(player)
            .cloned()
            .unwrap_or_default();
        let slip_ratio = contact_slip_ratio(
            footsteps.last_direction,
            travel_direction,
            horizontal_speed,
            friction,
            ground.slope_radians,
        );

        let mut emitted = Vec::<(PlayerEventKind, String)>::new();
        footsteps.scuff_cooldown = (footsteps.scuff_cooldown - dt).max(0.0);
        let model_contact_resolution = update_model_foot_contacts(
            world,
            player,
            ground,
            tuning.contact_skin,
            dt,
            &mut footsteps.model_contacts,
        );

        if ground.grounded {
            if !locomotion.was_grounded {
                emitted.push((
                    PlayerEventKind::GroundStateChanged,
                    format!(
                        "grounded surface='{}' slope_deg={:.1}",
                        surface.id,
                        ground.slope_radians.to_degrees()
                    ),
                ));
                let normal_impact_speed = landing_normal_impact_speed(
                    locomotion.max_downward_speed,
                    horizontal_velocity,
                    ground.normal,
                );
                if locomotion.airborne_time > landing_min_airborne_seconds
                    && normal_impact_speed >= tuning.landing_speed_threshold
                {
                    let sequence = footsteps.advance_sequence();
                    let position = landing_position(world, player, ground);
                    publish_authored_surface_event(
                        world,
                        player,
                        &surface,
                        "landing",
                        serde_json::json!({
                            "source_kind": "character_locomotion",
                            "phase": FootstepPhase::Land.slug(),
                            "mode": FootstepLocomotionMode::Land.slug(),
                            "surface": surface.id,
                            "surface_entity": ground_entity.map(EntityId::stable_u64),
                            "position": position,
                            "sequence": sequence,
                            "vertical_speed": locomotion.max_downward_speed,
                            "normal_impact_speed": normal_impact_speed,
                            "horizontal_speed": horizontal_speed,
                            "friction": friction,
                            "slope_radians": ground.slope_radians,
                        }),
                    );
                    emitted.push((
                        PlayerEventKind::Landed,
                        format!(
                            "event='{}' surface='{}' mode='land' vertical_speed={:.2} normal_impact={:.2} friction={:.2} slope_deg={:.1}",
                            surface.event_for("landing").unwrap_or(""),
                            surface.id,
                            locomotion.max_downward_speed,
                            normal_impact_speed,
                            friction,
                            ground.slope_radians.to_degrees(),
                        ),
                    ));
                    locomotion.step_distance = 0.0;
                    footsteps.was_moving = false;
                    footsteps.last_direction = Vec2::ZERO;
                }
            }

            let moving = dt > 0.0 && horizontal_speed > min_horizontal_speed;
            if moving {
                let stride = contact_stride(tuning.footstep_stride, mode);

                let scuff_side = footsteps.next_foot;
                let scuff_surface_key = model_contact_resolution
                    .and_then(|resolution| resolution.surface_key(scuff_side))
                    .or(ground.ground_entity);
                let scuff_slope_radians = model_contact_resolution
                    .map(|resolution| resolution.slope_radians(scuff_side))
                    .unwrap_or(ground.slope_radians);
                let (scuff_ground_entity, scuff_surface) =
                    resolve_contact_surface(world, key_to_entity, &surface, scuff_surface_key);
                let scuff_friction = surface_friction(world, scuff_ground_entity);
                let scuff_slip = contact_slip_ratio(
                    footsteps.last_direction,
                    travel_direction,
                    horizontal_speed,
                    scuff_friction,
                    scuff_slope_radians,
                );

                if footsteps.scuff_cooldown <= 0.0
                    && footsteps.was_moving
                    && (is_sharp_direction_change(footsteps.last_direction, travel_direction)
                        || scuff_slip >= 0.42)
                    && horizontal_speed > min_horizontal_speed.max(0.35)
                {
                    let sequence = footsteps.advance_sequence();
                    let position = phase_foot_position(
                        world,
                        player,
                        scuff_side,
                        ground,
                        FootstepPhase::Scuff,
                    );
                    publish_authored_surface_event(
                        world,
                        player,
                        &scuff_surface,
                        "scuff",
                        serde_json::json!({
                            "source_kind": "character_locomotion",
                            "phase": FootstepPhase::Scuff.slug(),
                            "mode": mode.slug(),
                            "foot": scuff_side.slug(),
                            "surface": scuff_surface.id,
                            "surface_entity": scuff_ground_entity.map(EntityId::stable_u64),
                            "position": position,
                            "sequence": sequence,
                            "speed": horizontal_speed,
                            "friction": scuff_friction,
                            "slip": scuff_slip,
                            "slope_radians": scuff_slope_radians,
                        }),
                    );
                    footsteps.scuff_cooldown = 0.22;
                }

                // Rigged models use animated foot/ground contact edges as cadence truth. The
                // distance accumulator is only a compatibility fallback for unrigged models.
                let mut contact_triggers =
                    Vec::<(FootSide, [f32; 3], &'static str, f32, f32, Option<u64>, f32)>::new();
                if let Some(resolution) = model_contact_resolution {
                    locomotion.step_distance = 0.0;
                    for (side, contact) in [
                        (FootSide::Left, resolution.frame.left),
                        (FootSide::Right, resolution.frame.right),
                    ] {
                        if contact.began {
                            let p = contact.point_world;
                            contact_triggers.push((
                                side,
                                [p.x, p.y, p.z],
                                "model-contact",
                                contact.normal_speed,
                                contact.signed_distance,
                                resolution.surface_key(side),
                                resolution.slope_radians(side),
                            ));
                            footsteps.next_foot = side.opposite();
                        }
                    }
                } else {
                    if footsteps.last_mode.is_some_and(|previous| previous != mode) {
                        locomotion.step_distance = locomotion.step_distance.min(stride * 0.45);
                    }
                    if !footsteps.was_moving {
                        locomotion.step_distance = locomotion.step_distance.max(stride * 0.55);
                    }
                    locomotion.step_distance += horizontal_speed * dt;

                    let mut contacts_this_tick = 0_u32;
                    while locomotion.step_distance >= stride && contacts_this_tick < 2 {
                        locomotion.step_distance -= stride;
                        contacts_this_tick += 1;
                        let side = footsteps.next_foot;
                        footsteps.next_foot = side.opposite();
                        contact_triggers.push((
                            side,
                            phase_foot_position(
                                world,
                                player,
                                side,
                                ground,
                                FootstepPhase::Contact,
                            ),
                            "distance-fallback",
                            0.0,
                            0.0,
                            ground.ground_entity,
                            ground.slope_radians,
                        ));
                    }
                }

                for (
                    side,
                    position,
                    contact_source,
                    normal_speed,
                    contact_distance,
                    contact_surface_key,
                    contact_slope_radians,
                ) in contact_triggers
                {
                    let (contact_ground_entity, contact_surface) = resolve_contact_surface(
                        world,
                        key_to_entity,
                        &surface,
                        contact_surface_key,
                    );
                    let contact_friction = surface_friction(world, contact_ground_entity);
                    let contact_slip = contact_slip_ratio(
                        footsteps.last_direction,
                        travel_direction,
                        horizontal_speed,
                        contact_friction,
                        contact_slope_radians,
                    );
                    let sequence = footsteps.advance_sequence();
                    publish_authored_surface_event(
                        world,
                        player,
                        &contact_surface,
                        "contact",
                        serde_json::json!({
                            "source_kind": "character_locomotion",
                            "phase": FootstepPhase::Contact.slug(),
                            "mode": mode.slug(),
                            "foot": side.slug(),
                            "surface": contact_surface.id,
                            "surface_entity": contact_ground_entity.map(EntityId::stable_u64),
                            "position": position,
                            "sequence": sequence,
                            "contact_source": contact_source,
                            "stride": stride,
                            "speed": horizontal_speed,
                            "contact_distance": contact_distance,
                            "normal_speed": normal_speed,
                            "friction": contact_friction,
                            "slip": contact_slip,
                            "slope_radians": contact_slope_radians,
                        }),
                    );
                    emitted.push((
                        PlayerEventKind::Footstep,
                        format!(
                            "event='{}' surface='{}' mode='{}' foot='{}' source='{}' stride={:.3} speed={:.2} contact_distance={:.4} normal_speed={:.3} friction={:.2} slip={:.3} slope_deg={:.1}",
                            contact_surface.event_for("contact").unwrap_or(""),
                            contact_surface.id,
                            mode.slug(),
                            side.slug(),
                            contact_source,
                            stride,
                            horizontal_speed,
                            contact_distance,
                            normal_speed,
                            contact_friction,
                            contact_slip,
                            contact_slope_radians.to_degrees(),
                        ),
                    ));
                }

                footsteps.last_direction = travel_direction;
                footsteps.last_mode = Some(mode);
                footsteps.was_moving = true;
            } else {
                locomotion.step_distance = 0.0;
                footsteps.last_direction = Vec2::ZERO;
                footsteps.last_mode = None;
                footsteps.was_moving = false;
            }

            if fall.airborne {
                if fall.falling {
                    let impact_speed = locomotion.max_downward_speed.max(fall.downward_speed);
                    landing = PlayerLandingState {
                        distance: fall.max_distance,
                        downward_speed: impact_speed,
                        horizontal_speed,
                        revision: landing.revision.saturating_add(1).max(1),
                    };
                    emitted.push((
                        PlayerEventKind::FallEnded,
                        format!(
                            "fall ended distance_m={:.3} peak_height={:.3} landing_height={:.3} max_downward_speed={:.3} horizontal_speed={:.3} landing_revision={}",
                            fall.max_distance,
                            fall.peak_height,
                            current_height,
                            impact_speed,
                            horizontal_speed,
                            landing.revision,
                        ),
                    ));
                }
                fall = PlayerFallState {
                    start_height: current_height,
                    peak_height: current_height,
                    current_height,
                    revision: fall.revision.saturating_add(1).max(1),
                    ..PlayerFallState::default()
                };
            } else {
                fall.start_height = current_height;
                fall.peak_height = current_height;
                fall.current_height = current_height;
                fall.distance = 0.0;
                fall.max_distance = 0.0;
                fall.downward_speed = 0.0;
            }
            locomotion.airborne_time = 0.0;
            locomotion.max_downward_speed = 0.0;
            locomotion.jump_started = false;
        } else {
            if locomotion.was_grounded {
                emitted.push((PlayerEventKind::GroundStateChanged, "airborne".to_owned()));
            }
            if !fall.airborne {
                fall.airborne = true;
                fall.falling = false;
                fall.start_height = current_height;
                fall.peak_height = current_height;
                fall.current_height = current_height;
                fall.distance = 0.0;
                fall.max_distance = 0.0;
                fall.downward_speed = 0.0;
                fall.revision = fall.revision.saturating_add(1).max(1);
            } else {
                fall.current_height = current_height;
                fall.peak_height = fall.peak_height.max(current_height);
                fall.distance = (fall.peak_height - current_height).max(0.0);
                fall.max_distance = fall.max_distance.max(fall.distance);
                fall.downward_speed = if velocity.y.is_finite() {
                    (-velocity.y).max(0.0)
                } else {
                    0.0
                };
                fall.revision = fall.revision.saturating_add(1).max(1);
                if !fall.falling
                    && player_fall_is_confirmed(
                        locomotion.jump_started,
                        locomotion.airborne_time,
                        velocity.y,
                    )
                {
                    fall.falling = true;
                    emitted.push((
                        PlayerEventKind::FallStarted,
                        format!(
                            "fall started start_height={:.3} peak_height={:.3} current_height={:.3} distance_m={:.3} downward_speed={:.3} state_component='PlayerFallState'",
                            fall.start_height,
                            fall.peak_height,
                            fall.current_height,
                            fall.distance,
                            fall.downward_speed,
                        ),
                    ));
                }
            }
            locomotion.step_distance = 0.0;
            locomotion.airborne_time += dt;
            if velocity.y.is_finite() {
                locomotion.max_downward_speed =
                    locomotion.max_downward_speed.max((-velocity.y).max(0.0));
                if locomotion.jump_started
                    && locomotion.airborne_time > 2.5
                    && velocity.y.abs() < 1.0
                {
                    locomotion.jump_started = false;
                }
            }
            footsteps.last_direction = Vec2::ZERO;
            footsteps.last_mode = None;
            footsteps.was_moving = false;
        }

        locomotion.was_grounded = ground.grounded;
        let _ = world.insert(player, locomotion);
        let _ = world.insert(player, fall);
        let _ = world.insert(player, landing);
        let _ = world.insert(player, footsteps);

        let fall_event = if ground.grounded {
            "character.fall.inactive"
        } else if fall.falling {
            "character.fall.active"
        } else {
            "character.fall.pending"
        };
        if let Err(error) = emit_animation_state(
            world,
            player,
            "character.fall",
            fall_event,
            serde_json::json!({
                "airborne": fall.airborne,
                "falling": fall.falling,
                "distance": fall.distance,
                "max_distance": fall.max_distance,
                "downward_speed": fall.downward_speed,
                "revision": fall.revision,
            }),
        ) {
            newengine_ulog_api::ulog::warn!(
                "locomotion animation fall-state publish failed player={} err='{}'",
                player.stable_u64(),
                error
            );
        }
        if landing.revision > landing_revision_before {
            if let Err(error) = emit_animation_pulse(
                world,
                player,
                "character.landing",
                "character.landing.impact",
                serde_json::json!({
                    "distance": landing.distance,
                    "downward_speed": landing.downward_speed,
                    "horizontal_speed": landing.horizontal_speed,
                    "revision": landing.revision,
                }),
            ) {
                newengine_ulog_api::ulog::warn!(
                    "locomotion animation landing publish failed player={} err='{}'",
                    player.stable_u64(),
                    error
                );
            }
        }

        for (kind, message) in emitted {
            emit_player_event(world, player, kind, message);
        }

        let _ = movement;
        let _ = slip_ratio;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{
        spawn_default_player, PlayerEventBus, PlayerFallState, PlayerLandingState,
        PlayerMovementSpeeds, PlayerStanceKind, PlayerStanceState,
    };
    use newengine_math::Vec3;

    #[test]
    fn authored_surface_signal_publishes_exact_project_event_id() {
        let mut world = World::new();
        let player = world.spawn();
        let surface = PhysicsSurface {
            id: "project.surface.custom".to_owned(),
            ..PhysicsSurface::default()
        }
        .with_event("contact", "project.events.boot_on_deck");
        publish_authored_surface_event(
            &mut world,
            player,
            &surface,
            "contact",
            serde_json::json!({"energy": 0.75}),
        );
        let events = newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "project.events.boot_on_deck");
        assert_eq!(events[0].source, Some(player.stable_u64()));
        assert_eq!(events[0].payload["energy"], 0.75);
    }

    #[test]
    fn unbound_surface_signal_is_silent_instead_of_inventing_event_id() {
        let mut world = World::new();
        let player = world.spawn();
        publish_authored_surface_event(
            &mut world,
            player,
            &PhysicsSurface::default(),
            "contact",
            serde_json::json!({"ignored": true}),
        );
        assert!(newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world).is_empty());
    }

    fn grounded_player(world: &mut World, velocity: Vec3) -> EntityId {
        let player = spawn_default_player(world, None, "footstep-test", Vec3::new(0.0, 1.0, 0.0));
        let _ = world.insert(player, Velocity(velocity));
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.grounded = true;
            ground.walkable = true;
            ground.last_fixed_tick = 1;
        }
        let _ = world.insert(
            player,
            PlayerMovementSpeeds {
                walk: 2.0,
                run: 5.0,
                sprint: 9.0,
                crouch: 1.6,
            },
        );
        player
    }

    #[test]
    fn walking_probe_glitches_never_manufacture_fall_or_landing_over_1000_frames() {
        use newengine_engine_runtime::gameplay::{
            drain_player_events, update_player_animation_states, PlayerAnimationState,
            PlayerLocomotionAnimation,
        };

        let mut world = World::new();
        let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -2.0));
        update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);
        update_player_animation_states(&mut world, 1.0 / 60.0);
        let _ = drain_player_events(&mut world);

        for frame in 0..1000 {
            let glitch = frame % 17 == 5 || frame % 17 == 6 || frame % 17 == 7;
            if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
                ground.grounded = !glitch;
                ground.walkable = !glitch;
                if !glitch {
                    ground.last_fixed_tick = ground.last_fixed_tick.saturating_add(1).max(1);
                }
            }
            if let Some(velocity) = world.get_mut::<Velocity>(player) {
                velocity.0 = Vec3::new(0.0, if glitch { -0.18 } else { 0.0 }, -2.0);
            }
            if glitch {
                if let Some(transform) = world.get_mut::<Transform>(player) {
                    transform.position.y -= 0.0005;
                }
            }

            update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);
            update_player_animation_states(&mut world, 1.0 / 60.0);

            let fall = world
                .get::<PlayerFallState>(player)
                .copied()
                .unwrap_or_default();
            assert!(
                !fall.falling,
                "probe glitch manufactured Fall at frame {frame}: {fall:?}"
            );
            let animation = world
                .get::<PlayerAnimationState>(player)
                .copied()
                .expect("player animation state");
            assert_ne!(
                animation.locomotion,
                PlayerLocomotionAnimation::Fall,
                "probe glitch selected Fall animation at frame {frame}"
            );
        }

        let events = drain_player_events(&mut world);
        assert!(!events.iter().any(|event| matches!(
            event.kind,
            PlayerEventKind::FallStarted | PlayerEventKind::FallEnded
        )));
        assert_eq!(
            world
                .get::<PlayerLandingState>(player)
                .copied()
                .unwrap_or_default()
                .revision,
            0,
            "probe glitches must not synthesize landing revisions"
        );
    }

    #[test]
    fn falling_publishes_height_aware_lifecycle_for_animation_subscribers() {
        let mut world = World::new();
        let player = grounded_player(&mut world, Vec3::ZERO);
        if let Some(transform) = world.get_mut::<Transform>(player) {
            transform.position.y = 10.0;
        }
        update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);

        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.grounded = false;
            ground.walkable = false;
        }
        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0.y = 3.0;
        }
        update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);
        if let Some(transform) = world.get_mut::<Transform>(player) {
            transform.position.y = 12.0;
        }
        update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);

        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0.y = -6.0;
        }
        // A walk-off/physics fall needs sustained airborne evidence; a single downward tick is
        // deliberately insufficient because ground-probe chatter can produce the same signal.
        for step in 0..4 {
            if let Some(transform) = world.get_mut::<Transform>(player) {
                transform.position.y = 12.0 - (step as f32 + 1.0) * 0.875;
            }
            update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
        }
        // The confirmation predicate observes airborne time accumulated by prior fixed steps.
        // Hold the same measured height for one more tick so this test crosses 0.35 s without
        // manufacturing extra fall distance.
        update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);

        let fall = world
            .get::<PlayerFallState>(player)
            .copied()
            .expect("fall state");
        assert!(fall.airborne && fall.falling);
        assert!((fall.peak_height - 12.0).abs() < 1.0e-4);
        assert!((fall.distance - 3.5).abs() < 1.0e-4);
        assert!(fall.downward_speed >= 6.0);
        let bus = world
            .resource::<PlayerEventBus>()
            .expect("player event bus");
        assert!(bus.events.iter().any(|event| {
            event.entity == player
                && event.kind == PlayerEventKind::FallStarted
                && event.message.contains("distance_m=3.500")
                && event.message.contains("state_component='PlayerFallState'")
        }));

        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.grounded = true;
            ground.walkable = true;
        }
        if let Some(transform) = world.get_mut::<Transform>(player) {
            transform.position.y = 8.0;
        }
        update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);
        let bus = world
            .resource::<PlayerEventBus>()
            .expect("player event bus");
        assert!(bus.events.iter().any(|event| {
            event.entity == player
                && event.kind == PlayerEventKind::FallEnded
                && event.message.contains("distance_m=3.500")
        }));
        let landing = world
            .get::<PlayerLandingState>(player)
            .copied()
            .expect("landing state");
        assert!((landing.distance - 3.5).abs() < 1.0e-4);
        assert!(landing.downward_speed >= 6.0);
        assert!(landing.revision > 0);
    }

    #[test]
    fn fixed_step_contacts_alternate_left_and_right() {
        let mut world = World::new();
        let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -6.0));
        update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
        update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
        update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
        update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
        let bus = world
            .resource::<PlayerEventBus>()
            .expect("player event bus");
        let contacts = bus
            .events
            .iter()
            .filter(|event| event.entity == player && event.kind == PlayerEventKind::Footstep)
            .collect::<Vec<_>>();
        assert!(
            contacts.len() >= 2,
            "expected alternating foot contacts: {contacts:?}"
        );
        assert!(contacts[0].message.contains("foot='left'"));
        assert!(contacts[1].message.contains("foot='right'"));
    }

    #[test]
    fn crouched_contact_reports_stealth_gait() {
        let mut world = World::new();
        let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -2.0));
        if let Some(stance) = world.get_mut::<PlayerStanceState>(player) {
            stance.current = PlayerStanceKind::Crouched;
        }
        for _ in 0..4 {
            update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
        }
        let bus = world
            .resource::<PlayerEventBus>()
            .expect("player event bus");
        assert!(bus.events.iter().any(|event| {
            event.entity == player
                && event.kind == PlayerEventKind::Footstep
                && event.message.contains("mode='stealth'")
        }));
    }
    #[test]
    fn rigged_model_cadence_is_driven_by_foot_contact_not_distance() {
        use newengine_engine_runtime::gameplay::{CollisionShapeDesc, PhysicsBodyDesc};
        use newengine_model_contact_api::ModelFootPoseState;
        use newengine_transform::Transform;

        let mut world = World::new();
        let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -6.0));
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.distance = 0.0;
            ground.normal = Vec3::Y;
        }

        let transform = world
            .get::<Transform>(player)
            .copied()
            .expect("player transform");
        let body = world
            .get::<PhysicsBodyDesc>(player)
            .copied()
            .expect("player physics body");
        let extent = match body.shape.sanitized() {
            CollisionShapeDesc::Box { half_extents } => half_extents[1],
            CollisionShapeDesc::Sphere { radius } => radius,
            CollisionShapeDesc::Capsule {
                radius,
                half_height,
            } => radius + half_height,
            CollisionShapeDesc::Cylinder { half_height, .. } => half_height,
        };
        let contact_skin = tuning(&world).contact_skin;
        let epsilon = (contact_skin.abs() * 0.25).clamp(0.001, 0.01);
        let support_y = transform.position.y - extent + epsilon;

        let high = ModelFootPoseState::from_world_positions(
            1,
            Vec3::new(-0.15, support_y + 0.14, 0.0),
            Vec3::new(0.15, support_y + 0.14, 0.0),
            None,
            0.1,
        );
        let _ = world.insert(player, high);

        // At 6 m/s the old stride accumulator would have emitted several contacts by now.
        for _ in 0..4 {
            update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
        }
        assert!(
            world
                .resource::<PlayerEventBus>()
                .expect("player event bus")
                .events
                .iter()
                .all(|event| event.kind != PlayerEventKind::Footstep),
            "distance must not manufacture footsteps while animated feet remain airborne"
        );

        let planted = ModelFootPoseState::from_world_positions(
            2,
            Vec3::new(-0.15, support_y + 0.03, 0.0),
            Vec3::new(0.15, support_y + 0.14, 0.0),
            Some(high),
            0.1,
        );
        let _ = world.insert(player, planted);
        update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);

        let bus = world
            .resource::<PlayerEventBus>()
            .expect("player event bus");
        let contacts = bus
            .events
            .iter()
            .filter(|event| event.entity == player && event.kind == PlayerEventKind::Footstep)
            .collect::<Vec<_>>();
        assert_eq!(
            contacts.len(),
            1,
            "one animated foot plant must emit one contact"
        );
        assert!(contacts[0].message.contains("source='model-contact'"));
        assert!(contacts[0].message.contains("foot='left'"));
    }
    #[test]
    fn rigged_feet_select_independent_surface_profiles() {
        use newengine_engine_runtime::gameplay::{CollisionShapeDesc, PhysicsBodyDesc};
        use newengine_model_contact_api::{
            ModelFootGroundSample, ModelFootGroundState, ModelFootPoseState, ModelGroundPlane,
        };
        use newengine_transform::Transform;

        let mut world = World::new();
        let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -4.0));
        let wood = world.spawn();
        let stone = world.spawn();
        let _ = world.insert(
            wood,
            PhysicsSurface {
                id: "surface.wood".to_owned(),
                ..PhysicsSurface::default()
            }
            .with_event("contact", "project.contact.wood"),
        );
        let _ = world.insert(
            stone,
            PhysicsSurface {
                id: "surface.stone".to_owned(),
                ..PhysicsSurface::default()
            }
            .with_event("contact", "project.contact.stone"),
        );

        let transform = world
            .get::<Transform>(player)
            .copied()
            .expect("player transform");
        let body = world
            .get::<PhysicsBodyDesc>(player)
            .copied()
            .expect("player physics body");
        let extent = match body.shape.sanitized() {
            CollisionShapeDesc::Box { half_extents } => half_extents[1],
            CollisionShapeDesc::Sphere { radius } => radius,
            CollisionShapeDesc::Capsule {
                radius,
                half_height,
            } => radius + half_height,
            CollisionShapeDesc::Cylinder { half_height, .. } => half_height,
        };
        let contact_skin = tuning(&world).contact_skin;
        let epsilon = (contact_skin.abs() * 0.25).clamp(0.001, 0.01);
        let support_y = transform.position.y - extent + epsilon;
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.distance = 0.0;
            ground.normal = Vec3::Y;
            ground.ground_entity = Some(wood.stable_u64());
        }
        let _ = world.insert(
            player,
            ModelFootGroundState {
                revision: 1,
                left: ModelFootGroundSample {
                    plane: ModelGroundPlane::new(Vec3::new(-0.15, support_y, 0.0), Vec3::Y),
                    surface_key: Some(wood.stable_u64()),
                },
                right: ModelFootGroundSample {
                    plane: ModelGroundPlane::new(Vec3::new(0.15, support_y, 0.0), Vec3::Y),
                    surface_key: Some(stone.stable_u64()),
                },
            },
        );
        let keys = BTreeMap::from([(wood.stable_u64(), wood), (stone.stable_u64(), stone)]);

        let high = ModelFootPoseState::from_world_positions(
            1,
            Vec3::new(-0.15, support_y + 0.14, 0.0),
            Vec3::new(0.15, support_y + 0.14, 0.0),
            None,
            0.1,
        );
        let _ = world.insert(player, high);
        update_player_locomotion(&mut world, &keys, 0.1);

        let left_plant = ModelFootPoseState::from_world_positions(
            2,
            Vec3::new(-0.15, support_y + 0.03, 0.0),
            Vec3::new(0.15, support_y + 0.14, 0.0),
            Some(high),
            0.1,
        );
        let _ = world.insert(player, left_plant);
        update_player_locomotion(&mut world, &keys, 0.1);

        let lifted = ModelFootPoseState::from_world_positions(
            3,
            Vec3::new(-0.15, support_y + 0.14, 0.0),
            Vec3::new(0.15, support_y + 0.14, 0.0),
            Some(left_plant),
            0.1,
        );
        let _ = world.insert(player, lifted);
        update_player_locomotion(&mut world, &keys, 0.1);

        let right_plant = ModelFootPoseState::from_world_positions(
            4,
            Vec3::new(-0.15, support_y + 0.14, 0.0),
            Vec3::new(0.15, support_y + 0.03, 0.0),
            Some(lifted),
            0.1,
        );
        let _ = world.insert(player, right_plant);
        update_player_locomotion(&mut world, &keys, 0.1);

        let bus = world
            .resource::<PlayerEventBus>()
            .expect("player event bus");
        let contacts = bus
            .events
            .iter()
            .filter(|event| event.entity == player && event.kind == PlayerEventKind::Footstep)
            .collect::<Vec<_>>();
        assert_eq!(
            contacts.len(),
            2,
            "expected one contact per planted foot: {contacts:?}"
        );
        assert!(contacts[0].message.contains("foot='left'"));
        assert!(contacts[0].message.contains("surface='surface.wood'"));
        assert!(contacts[1].message.contains("foot='right'"));
        assert!(contacts[1].message.contains("surface='surface.stone'"));
    }
}
