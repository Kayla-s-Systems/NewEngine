use super::*;

pub fn step_player_combat(world: &mut World, dt: f32, _fixed_tick: u64) {
    let gameplay_policy = world
        .resource::<FpsGameplayPolicySnapshot>()
        .cloned()
        .unwrap_or_default();
    let combat_policy = gameplay_policy.combat;
    let player_policy = gameplay_policy.player;
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let players = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .collect::<Vec<_>>();

    for player in players {
        // Equipment is authoritative for the active weapon and reserve ammunition.
        // Legacy direct weapon components remain supported when no inventory binding exists.
        sync_equipped_weapon_runtime(world, player);
        let actions = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| FpsActionFrame::from_commands(&commands.actions))
            .unwrap_or_default();
        let source_frame = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| commands.source_frame)
            .unwrap_or(0);
        let tuning = world
            .get::<HitscanWeaponTuning>(player)
            .copied()
            .unwrap_or_default()
            .sanitized();
        if world.get::<PlayerWeaponState>(player).is_none() {
            let _ = world.insert(player, PlayerWeaponState::loaded(tuning));
        }

        let inventory_backed = world.get::<EquippedWeaponBinding>(player).is_some();
        let mut state = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .unwrap_or_else(|| PlayerWeaponState::loaded(tuning));
        if let Some(reserve) = equipped_reserve_ammo(world, player) {
            state.reserve_ammo = reserve;
        }

        let mut events = Vec::<WeaponEvent>::new();
        let mut fire_request = None;
        state.aiming = actions.aim_held;
        state.cooldown_remaining = (state.cooldown_remaining - dt).max(0.0);

        if state.reload_remaining > 0.0 {
            state.reload_remaining = (state.reload_remaining - dt).max(0.0);
            if state.reload_remaining == 0.0 {
                let needed = tuning
                    .magazine_capacity
                    .saturating_sub(state.ammo_in_magazine);
                let moved = if inventory_backed {
                    consume_equipped_ammo(world, player, needed)
                } else {
                    needed.min(state.reserve_ammo)
                };
                state.ammo_in_magazine += moved;
                if inventory_backed {
                    state.reserve_ammo = equipped_reserve_ammo(world, player).unwrap_or(0);
                } else {
                    state.reserve_ammo -= moved;
                }
                events.push(weapon_event(
                    WeaponEventKind::ReloadCompleted,
                    player,
                    state.shot_sequence,
                ));
            }
        }

        if combat_policy.allow_reload
            && actions.reload_pressed
            && state.reload_remaining <= 0.0
            && state.ammo_in_magazine < tuning.magazine_capacity
            && state.reserve_ammo > 0
        {
            state.reload_remaining = tuning.reload_duration;
            events.push(weapon_event(
                WeaponEventKind::ReloadStarted,
                player,
                state.shot_sequence,
            ));
        }

        if combat_policy.allow_fire
            && actions.fire_primary_held
            && state.reload_remaining <= 0.0
            && state.cooldown_remaining <= 0.0
        {
            if state.ammo_in_magazine == 0 {
                if !state.empty_latched {
                    events.push(weapon_event(
                        WeaponEventKind::Empty,
                        player,
                        state.shot_sequence,
                    ));
                    state.empty_latched = true;
                }
            } else {
                state.ammo_in_magazine -= 1;
                state.shot_sequence = state.shot_sequence.wrapping_add(1);
                state.cooldown_remaining = tuning.fire_interval;
                state.empty_latched = false;
                fire_request = Some((state.shot_sequence, state.aiming));
                events.push(weapon_event(
                    WeaponEventKind::Fired,
                    player,
                    state.shot_sequence,
                ));
            }
        } else if !actions.fire_primary_held {
            state.empty_latched = false;
        }

        let _ = world.insert(player, state);
        persist_equipped_weapon_state(world, player);

        if let Some((shot_sequence, aiming)) = fire_request {
            if let Some((origin, direction)) =
                shot_origin_and_direction(world, player, tuning, aiming, shot_sequence)
            {
                let pending = PendingHitscan {
                    query_seq: hitscan_query_seq(player, shot_sequence),
                    shot_sequence,
                    origin,
                    direction,
                    range: tuning.range,
                    damage: tuning.damage * combat_policy.damage_multiplier,
                };
                let _ = world.insert(player, pending);
                apply_recoil(world, player, tuning, shot_sequence);
            }
        }

        if player_policy.allow_interact && actions.interact_pressed {
            let interaction_tuning = world
                .get::<PlayerInteractionTuning>(player)
                .copied()
                .unwrap_or_default();
            if let Some((origin, direction)) = interaction_ray(world, player, interaction_tuning) {
                let _ = world.insert(
                    player,
                    PendingInteraction {
                        query_seq: interaction_query_seq(player, source_frame),
                        origin,
                        direction,
                        range: (interaction_tuning.range
                            * combat_policy.interaction_range_multiplier)
                            .clamp(0.1, 100.0),
                    },
                );
            }
        }

        for event in events {
            emit_weapon_event(world, event);
        }
    }
}

pub(super) fn apply_recoil(
    world: &mut World,
    player: EntityId,
    tuning: HitscanWeaponTuning,
    shot_sequence: u64,
) {
    let Some(motor) = world.get_mut::<CharacterMotor>(player) else {
        return;
    };
    let yaw_sign = if shot_sequence.is_multiple_of(2) {
        1.0
    } else {
        -1.0
    };
    let yaw_scale = 0.55 + signed_unit(shot_sequence ^ 0xa409_3822).abs() * 0.45;
    // Positive pitch rotates the engine forward vector (-Z) upward. Recoil therefore
    // increases pitch; subtracting it drives the crosshair down.
    motor.pitch =
        (motor.pitch + tuning.recoil_pitch_radians).clamp(-motor.pitch_limit, motor.pitch_limit);
    motor.yaw += tuning.recoil_yaw_radians * yaw_sign * yaw_scale;
}

fn weapon_event(kind: WeaponEventKind, shooter: EntityId, shot_sequence: u64) -> WeaponEvent {
    WeaponEvent {
        kind,
        shooter,
        target: None,
        shot_sequence,
        damage: 0.0,
        point: Vec3::ZERO,
        normal: Vec3::ZERO,
    }
}

pub(super) fn emit_weapon_event(world: &mut World, event: WeaponEvent) {
    if world.resource::<WeaponEventBus>().is_none() {
        world.insert_resource(WeaponEventBus::default());
    }
    if let Some(bus) = world.resource_mut::<WeaponEventBus>() {
        bus.emit(event);
    }
}

pub(super) fn emit_interaction_event(world: &mut World, event: InteractionEvent) {
    if world.resource::<InteractionEventBus>().is_none() {
        world.insert_resource(InteractionEventBus::default());
    }
    if let Some(bus) = world.resource_mut::<InteractionEventBus>() {
        bus.emit(event);
    }
}
