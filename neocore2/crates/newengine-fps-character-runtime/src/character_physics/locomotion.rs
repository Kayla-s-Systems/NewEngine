use std::collections::BTreeMap;

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    emit_player_event, PhysicsSurface, PlayerController, PlayerEventKind, PlayerFallState,
    PlayerGroundState, PlayerLandingState, PlayerLocomotionState, PlayerMovementSpeeds,
    StaticMeshCollider,
};
use newengine_math::Vec2;
use newengine_sim::{CharacterMotor, Velocity};
use newengine_transform::Transform;

use super::footsteps::{
    classify_player_footstep_mode, classify_surface, contact_modulation, contact_plan,
    contact_slip_ratio, contact_stride, is_sharp_direction_change, landing_modulation,
    landing_normal_impact_speed, landing_position, phase_foot_position, phase_seed,
    play_locomotion_action, resolve_footstep_cue, scuff_cue, surface_friction,
    update_model_foot_contacts, FootSide, FootstepAudioAction, FootstepLocomotionMode,
    FootstepPhase, FootstepRuntimeState, PendingFootstepAudio,
};
use super::tuning::tuning;

/// Runs FPS-owned contact-phase footstep and landing semantics after the physics provider has
/// resolved ground probes. Physics remains material/audio agnostic; this layer consumes only
/// provider-neutral velocity, grounding and `PhysicsSurface` identity.
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
        let surface_kind = classify_surface(&surface);
        let friction = surface_friction(world, ground_entity, surface_kind);
        let movement = world
            .get::<PlayerMovementSpeeds>(player)
            .copied()
            .unwrap_or_default();
        let mode = classify_player_footstep_mode(world, player, horizontal_speed);

        // Work on local copies so contact planning can read ordinary ECS components without
        // borrowing the world through PlayerLocomotionState/FootstepRuntimeState simultaneously.
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
        let contact_physics = contact_modulation(
            surface_kind,
            mode,
            horizontal_speed,
            movement,
            friction,
            ground.slope_radians,
            slip_ratio,
        );

        let mut emitted = Vec::<(PlayerEventKind, String)>::new();
        let mut audio_actions = Vec::<FootstepAudioAction>::new();
        footsteps.scuff_cooldown = (footsteps.scuff_cooldown - dt).max(0.0);
        // Advance the model-contact latch even while idle/airborne, so beginning to move does not
        // turn an already planted foot into a synthetic first step.
        let model_contact_resolution = update_model_foot_contacts(
            world,
            player,
            ground,
            tuning.contact_skin,
            dt,
            &mut footsteps.model_contacts,
        );

        if ground.grounded {
            // Secondary phases (toe/lift) remain phase-related to the originating foot contact
            // even if the character stopped immediately after planting that foot.
            audio_actions.extend(footsteps.tick_pending(dt));

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
                    // Landing supersedes any airborne leftovers and has its own priority/concurrency
                    // family. Both feet are represented by a centered foot-sole emitter position.
                    footsteps.cancel_pending();
                    let sequence = footsteps.advance_sequence();
                    let cue = resolve_footstep_cue(&surface, FootstepLocomotionMode::Land);
                    let landing = landing_modulation(
                        surface_kind,
                        normal_impact_speed,
                        tuning.landing_speed_threshold,
                    );
                    let action = FootstepAudioAction {
                        cue: cue.clone(),
                        position: landing_position(world, player, ground),
                        gain: landing.gain,
                        pitch: landing.pitch,
                        seed: phase_seed(
                            player,
                            sequence,
                            surface_kind,
                            FootstepLocomotionMode::Land,
                            FootstepPhase::Land,
                            None,
                        ),
                        phase: FootstepPhase::Land,
                        foot: None,
                    };
                    audio_actions.push(action);
                    emitted.push((
                        PlayerEventKind::Landed,
                        format!(
                            "cue='{}' event='{}' surface='{}' mode='land' vertical_speed={:.2} normal_impact={:.2} gain={:.3} pitch={:.3} friction={:.2} slope_deg={:.1}",
                            cue,
                            surface.landing_event,
                            surface.id,
                            locomotion.max_downward_speed,
                            landing.normal_impact_speed,
                            landing.gain,
                            landing.pitch,
                            friction,
                            ground.slope_radians.to_degrees(),
                        ),
                    ));
                    // Start the next locomotion cycle cleanly after an impact.
                    locomotion.step_distance = 0.0;
                    footsteps.was_moving = false;
                    footsteps.last_direction = Vec2::ZERO;
                }
            }

            let moving = dt > 0.0 && horizontal_speed > min_horizontal_speed;
            if moving {
                let stride =
                    contact_stride(tuning.footstep_stride, mode) * contact_physics.stride_scale;

                let resolve_contact_surface = |surface_key: Option<u64>| {
                    let entity = surface_key.and_then(|key| key_to_entity.get(&key).copied());
                    let semantic = entity
                        .and_then(|entity| world.get::<PhysicsSurface>(entity).cloned())
                        .unwrap_or_else(|| {
                            if surface_key.is_none() {
                                surface.clone()
                            } else {
                                PhysicsSurface::default()
                            }
                        });
                    (entity, semantic)
                };

                // Scuff belongs to the supporting foot too. When per-foot probes exist, use that
                // foot's material/friction/slope instead of the capsule-center surface.
                let scuff_side = footsteps.next_foot;
                let scuff_surface_key = model_contact_resolution
                    .and_then(|resolution| resolution.surface_key(scuff_side))
                    .or(ground.ground_entity);
                let scuff_slope_radians = model_contact_resolution
                    .map(|resolution| resolution.slope_radians(scuff_side))
                    .unwrap_or(ground.slope_radians);
                let (scuff_ground_entity, scuff_surface) =
                    resolve_contact_surface(scuff_surface_key);
                let scuff_surface_kind = classify_surface(&scuff_surface);
                let scuff_friction =
                    surface_friction(world, scuff_ground_entity, scuff_surface_kind);
                let scuff_slip = contact_slip_ratio(
                    footsteps.last_direction,
                    travel_direction,
                    horizontal_speed,
                    scuff_friction,
                    scuff_slope_radians,
                );
                let scuff_physics = contact_modulation(
                    scuff_surface_kind,
                    mode,
                    horizontal_speed,
                    movement,
                    scuff_friction,
                    scuff_slope_radians,
                    scuff_slip,
                );

                if footsteps.scuff_cooldown <= 0.0
                    && footsteps.was_moving
                    && (is_sharp_direction_change(footsteps.last_direction, travel_direction)
                        || scuff_slip >= 0.42)
                    && horizontal_speed > min_horizontal_speed.max(0.35)
                {
                    let sequence = footsteps.advance_sequence();
                    let cue = scuff_cue(&scuff_surface);
                    audio_actions.push(FootstepAudioAction {
                        cue: cue.clone(),
                        position: phase_foot_position(
                            world,
                            player,
                            scuff_side,
                            ground,
                            FootstepPhase::Scuff,
                        ),
                        gain: scuff_physics.scuff_gain,
                        pitch: (scuff_physics.pitch * (1.0 - scuff_slip * 0.035)).clamp(0.88, 1.08),
                        seed: phase_seed(
                            player,
                            sequence,
                            scuff_surface_kind,
                            mode,
                            FootstepPhase::Scuff,
                            Some(scuff_side),
                        ),
                        phase: FootstepPhase::Scuff,
                        foot: Some(scuff_side),
                    });
                    footsteps.scuff_cooldown = 0.22;
                }

                // Rigged models use animated foot/ground contact edges as cadence truth. The old
                // distance accumulator remains only as a compatibility fallback for unrigged models.
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
                    // Compatibility path for procedural/non-rigged characters only.
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
                    let (contact_ground_entity, contact_surface) =
                        resolve_contact_surface(contact_surface_key);
                    let contact_surface_kind = classify_surface(&contact_surface);
                    let contact_friction =
                        surface_friction(world, contact_ground_entity, contact_surface_kind);
                    let contact_slip = contact_slip_ratio(
                        footsteps.last_direction,
                        travel_direction,
                        horizontal_speed,
                        contact_friction,
                        contact_slope_radians,
                    );
                    let contact_physics = contact_modulation(
                        contact_surface_kind,
                        mode,
                        horizontal_speed,
                        movement,
                        contact_friction,
                        contact_slope_radians,
                        contact_slip,
                    );
                    // Foot normal velocity is animation-derived physical evidence. Keep its gain
                    // contribution deliberately bounded so pose jitter can never spike a sample.
                    let impact_gain_scale = if contact_source == "model-contact" {
                        (0.94 + (-normal_speed).clamp(0.0, 3.0) * 0.04).clamp(0.94, 1.06)
                    } else {
                        1.0
                    };
                    let contact_gain = (contact_physics.gain * impact_gain_scale).clamp(0.25, 1.35);

                    let sequence = footsteps.advance_sequence();
                    let plan = contact_plan(&contact_surface, mode);
                    let has_toe = plan.toe_cue.is_some();
                    let has_lift = plan.lift_cue.is_some();
                    audio_actions.push(FootstepAudioAction {
                        cue: plan.primary_cue.clone(),
                        position,
                        gain: contact_gain,
                        pitch: contact_physics.pitch,
                        seed: phase_seed(
                            player,
                            sequence,
                            contact_surface_kind,
                            mode,
                            FootstepPhase::Contact,
                            Some(side),
                        ),
                        phase: FootstepPhase::Contact,
                        foot: Some(side),
                    });

                    if let Some(cue) = plan.toe_cue {
                        footsteps.pending.push(PendingFootstepAudio {
                            remaining_seconds: plan.toe_delay_seconds,
                            action: FootstepAudioAction {
                                cue,
                                position: phase_foot_position(
                                    world,
                                    player,
                                    side,
                                    ground,
                                    FootstepPhase::Toe,
                                ),
                                gain: (plan.toe_gain * contact_gain).clamp(0.30, 1.10),
                                pitch: (contact_physics.pitch * 1.008).clamp(0.88, 1.12),
                                seed: phase_seed(
                                    player,
                                    sequence,
                                    contact_surface_kind,
                                    mode,
                                    FootstepPhase::Toe,
                                    Some(side),
                                ),
                                phase: FootstepPhase::Toe,
                                foot: Some(side),
                            },
                        });
                    }
                    if let Some(cue) = plan.lift_cue {
                        footsteps.pending.push(PendingFootstepAudio {
                            remaining_seconds: plan.lift_delay_seconds,
                            action: FootstepAudioAction {
                                cue,
                                position: phase_foot_position(
                                    world,
                                    player,
                                    side,
                                    ground,
                                    FootstepPhase::Lift,
                                ),
                                gain: (plan.lift_gain * contact_gain).clamp(0.28, 1.05),
                                pitch: (contact_physics.pitch * 1.012).clamp(0.88, 1.12),
                                seed: phase_seed(
                                    player,
                                    sequence,
                                    contact_surface_kind,
                                    mode,
                                    FootstepPhase::Lift,
                                    Some(side),
                                ),
                                phase: FootstepPhase::Lift,
                                foot: Some(side),
                            },
                        });
                    }

                    emitted.push((
                        PlayerEventKind::Footstep,
                        format!(
                            "cue='{}' event='{}' surface='{}' mode='{}' foot='{}' source='{}' stride={:.3} speed={:.2} contact_distance={:.4} normal_speed={:.3} gain={:.3} pitch={:.3} friction={:.2} slip={:.3} slope_deg={:.1} toe={} lift={}",
                            plan.primary_cue,
                            contact_surface.footstep_event,
                            contact_surface.id,
                            mode.slug(),
                            side.slug(),
                            contact_source,
                            stride,
                            horizontal_speed,
                            contact_distance,
                            normal_speed,
                            contact_gain,
                            contact_physics.pitch,
                            contact_friction,
                            contact_slip,
                            contact_slope_radians.to_degrees(),
                            has_toe,
                            has_lift,
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
                    && velocity.y.is_finite()
                    && velocity.y < 0.0
                    && fall.distance > 1.0e-4
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
            // Toe/lift sounds from the previous grounded contact must never leak into airborne
            // time after a jump/fall transition.
            footsteps.cancel_pending();
            footsteps.last_direction = Vec2::ZERO;
            footsteps.last_mode = None;
            footsteps.was_moving = false;
        }

        locomotion.was_grounded = ground.grounded;
        let _ = world.insert(player, locomotion);
        let _ = world.insert(player, fall);
        let _ = world.insert(player, landing);
        let _ = world.insert(player, footsteps);

        for action in &audio_actions {
            play_locomotion_action(action);
        }
        for (kind, message) in emitted {
            emit_player_event(world, player, kind, message);
        }
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

        if let Some(transform) = world.get_mut::<Transform>(player) {
            transform.position.y = 8.5;
        }
        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0.y = -6.0;
        }
        update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);

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
                footstep_event: "audio.footstep.wood".to_owned(),
                landing_event: "audio.landing.wood".to_owned(),
            },
        );
        let _ = world.insert(
            stone,
            PhysicsSurface {
                id: "surface.stone".to_owned(),
                footstep_event: "audio.footstep.stone".to_owned(),
                landing_event: "audio.landing.stone".to_owned(),
            },
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
