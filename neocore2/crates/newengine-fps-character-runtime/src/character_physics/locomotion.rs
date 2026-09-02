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
#[path = "locomotion/tests.rs"]
mod tests;
