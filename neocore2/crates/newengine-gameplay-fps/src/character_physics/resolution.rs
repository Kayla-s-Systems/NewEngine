use std::collections::{BTreeMap, BTreeSet};

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    apply_player_stance_geometry, emit_player_event, PhysicsBodyDesc, PlayerController,
    PlayerEventKind, PlayerGroundState, PlayerStanceKind, PlayerStanceState,
};
use newengine_math::Vec3;
use newengine_physics_api::PhysicsQueryHitDto;
use newengine_sim::{CharacterMotor, Velocity};

use super::queries::{ground_probe_max_t, stand_probe_owner};
use super::tuning::tuning;
use crate::game_data::active_game_data;

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
    let max_distance = ground_probe_max_t(tuning.contact_skin, tuning.ground_probe_distance);
    if !hit.distance.is_finite() || !(0.0..=max_distance).contains(&hit.distance) {
        return;
    }
    let vertical_velocity = world
        .get::<Velocity>(player)
        .map(|velocity| velocity.0.y)
        .unwrap_or(0.0);
    if !vertical_velocity.is_finite()
        || vertical_velocity
            > active_game_data(world)
                .player
                .tuning
                .ground_probe_max_upward_velocity
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{
        spawn_default_player, PlayerGroundState, PlayerLocomotionState,
    };
    use newengine_math::Vec3;

    fn player_key_map(player: EntityId) -> BTreeMap<u64, EntityId> {
        BTreeMap::from([(player.stable_u64(), player)])
    }

    fn walkable_hit(player: EntityId, ground: EntityId, distance: f32) -> PhysicsQueryHitDto {
        PhysicsQueryHitDto {
            seq: player.stable_u64(),
            entity: ground.stable_u64(),
            position: [0.0, -distance, 0.0],
            normal: [0.0, 1.0, 0.0],
            distance,
        }
    }

    #[test]
    fn ground_probe_contact_survives_two_missing_fixed_ticks_then_expires() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "ground-grace", Vec3::ZERO);
        let ground = world.spawn();
        let keys = player_key_map(player);

        resolve_character_query_hits(&mut world, 10, &[walkable_hit(player, ground, 0.02)], &keys);
        let confirmed = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground");
        assert!(confirmed.grounded);
        assert_eq!(confirmed.last_fixed_tick, 10);

        for fixed_tick in [11_u64, 12] {
            resolve_character_query_hits(&mut world, fixed_tick, &[], &keys);
            let retained = world
                .get::<PlayerGroundState>(player)
                .copied()
                .expect("ground");
            assert!(
                retained.grounded,
                "probe miss tick={fixed_tick} should be retained"
            );
            assert_eq!(retained.ground_entity, Some(ground.stable_u64()));
            assert_eq!(retained.last_fixed_tick, 10);
        }

        resolve_character_query_hits(&mut world, 13, &[], &keys);
        let expired = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground");
        assert!(!expired.grounded);
        assert!(!expired.walkable);
        assert_eq!(expired.ground_entity, None);
        assert_eq!(expired.last_fixed_tick, 10);
    }

    #[test]
    fn jump_is_accepted_during_single_ground_probe_gap() {
        use newengine_engine_runtime::gameplay::PlayerCommandFrame;
        use newengine_gameplay_fps_api::action;
        use newengine_input_actions_api::ActionCommandFrame;

        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "ground-gap-jump", Vec3::ZERO);
        let ground = world.spawn();
        let keys = player_key_map(player);

        resolve_character_query_hits(&mut world, 30, &[walkable_hit(player, ground, 0.02)], &keys);
        resolve_character_query_hits(&mut world, 31, &[], &keys);
        assert!(world
            .get::<PlayerGroundState>(player)
            .is_some_and(|state| state.grounded));

        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 501;
            commands.actions = ActionCommandFrame {
                pressed: vec![action::PLAYER_JUMP.into()],
                ..ActionCommandFrame::default()
            };
        }
        crate::character_control::apply_fps_character_commands(&mut world, 1.0 / 60.0, 31);

        let ground_state = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground");
        let locomotion = world
            .get::<PlayerLocomotionState>(player)
            .copied()
            .expect("locomotion");
        assert!(
            !ground_state.grounded,
            "accepted jump must clear retained ground immediately"
        );
        assert!(
            locomotion.jump_started,
            "accepted jump must publish explicit jump origin"
        );
        assert_eq!(locomotion.last_jump_command_source_frame, Some(501));
        assert!(world
            .get::<Velocity>(player)
            .is_some_and(|velocity| velocity.0.y > 0.0));

        resolve_character_query_hits(&mut world, 32, &[], &keys);
        assert!(
            !world
                .get::<PlayerGroundState>(player)
                .expect("ground")
                .grounded,
            "ground grace must not re-latch an explicit jump"
        );
    }

    #[test]
    fn explicit_jump_clear_is_not_relatched_by_probe_miss_grace() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "ground-jump-clear", Vec3::ZERO);
        let ground = world.spawn();
        let keys = player_key_map(player);
        resolve_character_query_hits(&mut world, 20, &[walkable_hit(player, ground, 0.02)], &keys);
        if let Some(state) = world.get_mut::<PlayerGroundState>(player) {
            state.grounded = false;
            state.walkable = false;
            state.ground_entity = None;
            state.distance = f32::INFINITY;
        }
        if let Some(state) = world.get_mut::<PlayerLocomotionState>(player) {
            state.jump_started = true;
        }

        resolve_character_query_hits(&mut world, 21, &[], &keys);
        let airborne = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground");
        assert!(
            !airborne.grounded,
            "explicit jump must bypass contact grace"
        );
        assert_eq!(airborne.ground_entity, None);
    }
}
