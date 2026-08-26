use super::*;

pub fn step_player_combat(world: &mut World, dt: f32, fixed_tick: u64) {
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
    crate::projectiles::step_weapon_shot_fx(world, dt);
    let players = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .collect::<Vec<_>>();

    for player in players {
        // Inventory/equipment is the only authority for firearm availability. Never synthesize
        // the old demo rifle state when the player has no equipped weapon.
        sync_equipped_weapon_runtime(world, player);
        let actions = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| FpsActionFrame::from_commands(&commands.actions))
            .unwrap_or_default();
        let source_frame = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| commands.source_frame)
            .unwrap_or(0);

        if world.get::<EquippedWeaponBinding>(player).is_some() {
            // Keep a fixed-step body-to-muzzle obstruction probe alive even when the trigger is
            // idle. Presentation consumes the previous resolved state, so the gun raises away from
            // a wall before the player fires rather than correcting after penetration.
            queue_weapon_obstruction_probe(world, player, fixed_tick);
            let tuning = world
                .get::<HitscanWeaponTuning>(player)
                .copied()
                .unwrap_or_default()
                .sanitized();
            if world.get::<PlayerWeaponState>(player).is_none() {
                let _ = world.insert(player, PlayerWeaponState::loaded(tuning));
            }

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
                    let moved = consume_equipped_ammo(world, player, needed);
                    state.ammo_in_magazine += moved;
                    state.reserve_ammo = equipped_reserve_ammo(world, player).unwrap_or(0);
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

            let fire_mode = world
                .get::<EquippedWeaponBinding>(player)
                .and_then(|binding| {
                    world
                        .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()
                        .and_then(|catalog| catalog.get(binding.item))
                })
                .and_then(|definition| definition.weapon)
                .map(|weapon| weapon.fire_mode)
                .unwrap_or(WeaponFireMode::SemiAuto);
            let trigger_active = match fire_mode {
                WeaponFireMode::SemiAuto => actions.fire_primary_pressed,
                WeaponFireMode::Automatic => actions.fire_primary_held,
            };

            if combat_policy.allow_fire
                && trigger_active
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
                    crate::projectiles::spawn_weapon_shot_fx(
                        world,
                        player,
                        shot_sequence,
                        origin,
                        direction,
                        tuning.range,
                    );
                    newengine_ulog_api::ulog::info!(
                        "weapon fire: shooter={} shot={} ammo_after={} origin={:?} direction={:?} muzzle_published={} fx='muzzle_flash+point_light+tracer'",
                        player.stable_u64(),
                        shot_sequence,
                        state.ammo_in_magazine,
                        origin,
                        direction,
                        world.get::<EquippedWeaponMuzzle>(player).is_some(),
                    );
                    apply_recoil(world, player, tuning, shot_sequence);
                } else {
                    newengine_ulog_api::ulog::warn!(
                        "weapon fire rejected after ammo consume: shooter={} shot={} reason='unable to resolve shot origin/direction'",
                        player.stable_u64(),
                        shot_sequence,
                    );
                }
            }

            for event in events {
                let action = match event.kind {
                    WeaponEventKind::Fired => Some(WeaponAudioAction::Fire),
                    WeaponEventKind::Empty => Some(WeaponAudioAction::Empty),
                    WeaponEventKind::ReloadStarted => Some(WeaponAudioAction::ReloadStart),
                    WeaponEventKind::ReloadCompleted => Some(WeaponAudioAction::ReloadComplete),
                    WeaponEventKind::Hit => None,
                };
                if let Some(action) = action {
                    play_equipped_weapon_audio(world, event.shooter, action);
                }
                emit_weapon_event(world, event);
            }
        } else {
            // LMB without a firearm is an edge-triggered unarmed strike. This uses the same
            // authoritative physics-query + Lua hit-policy pipeline as firearm damage, but with
            // close range and no projectile, ammo, recoil, reload, or synthetic weapon state.
            let mut melee = world
                .get::<UnarmedMeleeState>(player)
                .copied()
                .unwrap_or_default();
            melee.cooldown_remaining = (melee.cooldown_remaining - dt).max(0.0);
            if combat_policy.allow_fire
                && actions.fire_primary_pressed
                && melee.cooldown_remaining <= 0.0
            {
                melee.sequence = melee.sequence.wrapping_add(1).max(1);
                melee.cooldown_remaining = UNARMED_MELEE_COOLDOWN_SECONDS;
                let tuning = PlayerInteractionTuning {
                    range: UNARMED_MELEE_RANGE,
                    ray_origin_forward_offset: 0.20,
                };
                if let Some((origin, direction)) = interaction_ray(world, player, tuning) {
                    let _ = world.insert(
                        player,
                        PendingHitscan {
                            query_seq: melee_query_seq(player, melee.sequence),
                            shot_sequence: melee.sequence,
                            origin,
                            direction,
                            range: UNARMED_MELEE_RANGE,
                            damage: UNARMED_MELEE_DAMAGE * combat_policy.damage_multiplier,
                        },
                    );
                }
            }
            let _ = world.insert(player, melee);
        }

        if player_policy.allow_interact && actions.interact_pressed {
            if let Some(target) = focused_item_pickup(world, player) {
                let point = world
                    .get::<Transform>(target)
                    .map(|transform| transform.position)
                    .unwrap_or_default();
                let _ = world.insert(player, PendingFocusedItemInteraction { target, point });
                // A focused inventory item owns this interaction edge. Remove any stale generic
                // ray request so one key press cannot trigger both a pickup and a door/terminal.
                let _ = world.remove::<PendingInteraction>(player);
            } else {
                let interaction_tuning = world
                    .get::<PlayerInteractionTuning>(player)
                    .copied()
                    .unwrap_or_default();
                if let Some((origin, direction)) =
                    interaction_ray(world, player, interaction_tuning)
                {
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
