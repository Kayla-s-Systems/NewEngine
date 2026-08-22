use std::collections::BTreeMap;

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    emit_player_event, PhysicsSurface, PlayerController, PlayerEventKind, PlayerGroundState,
    PlayerLocomotionState,
};
use newengine_math::Vec2;
use newengine_sim::{CharacterMotor, Velocity};

use super::tuning::tuning;
use crate::game_data::active_game_data;

/// Runs FPS-only footstep/landing policy after the physics provider has resolved ground probes.
pub(crate) fn step_character_locomotion(world: &mut World, dt: f32) {
    // Ground locomotion only resolves entities that can contribute surface metadata.
    // Avoid rebuilding an index for every ECS entity on every frame.
    let key_to_entity = world
        .query::<PhysicsSurface>()
        .map(|(entity, _)| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    update_player_locomotion(world, &key_to_entity, dt);
}

fn update_player_locomotion(world: &mut World, key_to_entity: &BTreeMap<u64, EntityId>, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let tuning = tuning(world);
    let player_data = active_game_data(world).player.tuning;
    let min_horizontal_speed = player_data.locomotion_min_horizontal_speed;
    let landing_min_airborne_seconds = player_data.landing_min_airborne_seconds;
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
                    if state.airborne_time > landing_min_airborne_seconds
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

                if dt > 0.0 && horizontal_speed > min_horizontal_speed {
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
                state.jump_started = false;
            } else {
                if state.was_grounded {
                    emitted.push((PlayerEventKind::GroundStateChanged, "airborne".to_owned()));
                }
                state.step_distance = 0.0;
                state.airborne_time += dt;
                if velocity.y.is_finite() {
                    state.max_downward_speed = state.max_downward_speed.max((-velocity.y).max(0.0));
                    if state.jump_started && state.airborne_time > 2.5 && velocity.y.abs() < 1.0 {
                        state.jump_started = false;
                    }
                }
            }
            state.was_grounded = ground.grounded;
        }

        for (kind, message) in emitted {
            emit_player_event(world, player, kind, message);
        }
    }
}
