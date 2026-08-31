use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
struct WeaponRecoilRuntime {
    weapon_instance_id: ItemInstanceId,
    applied_pitch_radians: f32,
    applied_yaw_radians: f32,
    pitch_speed_radians_per_second: f32,
    yaw_speed_radians_per_second: f32,
    recovery_hz: f32,
}

#[inline]
fn critically_damped_recoil_step(angle: f32, speed: f32, recovery_hz: f32, dt: f32) -> (f32, f32) {
    // TLOU2 exposes both a recoil angle and a recoil-tracker speed. Model that contract as a
    // stable critically damped second-order tracker rather than subtracting a fixed exponential
    // fraction of the angle every frame. The analytic form is stable even on a long frame.
    let omega = recovery_hz.max(0.05) * 2.2;
    let c = speed + omega * angle;
    let decay = (-omega * dt).exp();
    let next_angle = (angle + c * dt) * decay;
    let next_speed = (speed - omega * c * dt) * decay;
    if next_angle.is_finite() && next_speed.is_finite() {
        (next_angle, next_speed)
    } else {
        (0.0, 0.0)
    }
}

pub(super) fn recover_weapon_recoil(world: &mut World, player: EntityId, dt: f32) {
    let Some(mut recoil) = world.get::<WeaponRecoilRuntime>(player).copied() else {
        return;
    };
    if dt <= 0.0 {
        return;
    }
    let (next_pitch, next_pitch_speed) = critically_damped_recoil_step(
        recoil.applied_pitch_radians,
        recoil.pitch_speed_radians_per_second,
        recoil.recovery_hz,
        dt,
    );
    let (next_yaw, next_yaw_speed) = critically_damped_recoil_step(
        recoil.applied_yaw_radians,
        recoil.yaw_speed_radians_per_second,
        recoil.recovery_hz,
        dt,
    );
    if let Some(motor) = world.get_mut::<CharacterMotor>(player) {
        motor.pitch = (motor.pitch + next_pitch - recoil.applied_pitch_radians)
            .clamp(-motor.pitch_limit, motor.pitch_limit);
        motor.yaw += next_yaw - recoil.applied_yaw_radians;
    }
    recoil.applied_pitch_radians = next_pitch;
    recoil.applied_yaw_radians = next_yaw;
    recoil.pitch_speed_radians_per_second = next_pitch_speed;
    recoil.yaw_speed_radians_per_second = next_yaw_speed;
    if next_pitch.abs() < 1.0e-5
        && next_yaw.abs() < 1.0e-5
        && next_pitch_speed.abs() < 1.0e-4
        && next_yaw_speed.abs() < 1.0e-4
    {
        let _ = world.remove::<WeaponRecoilRuntime>(player);
    } else {
        let _ = world.insert(player, recoil);
    }
}

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
    newengine_fps_projectile_runtime::step_weapon_shot_fx(world, dt);
    let players = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .collect::<Vec<_>>();

    for player in players {
        // Inventory/equipment is the only authority for firearm availability. Never synthesize
        // the old demo rifle state when the player has no equipped weapon.
        sync_equipped_weapon_runtime(world, player);
        recover_weapon_recoil(world, player, dt);
        let actions = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| FpsActionFrame::from_commands(&commands.actions))
            .unwrap_or_default();
        let source_frame = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| commands.source_frame)
            .unwrap_or(0);

        if let Some(binding) = active_equipped_weapon_binding(world, player) {
            let capabilities = binding.capabilities();
            let authored_animation = world
                .get::<PlayerAuthoredAnimationCapabilities>(player)
                .copied();
            let equipment_ready_supported =
                authored_animation.is_none_or(|value| value.equipment_ready);
            let equipment_aim_supported =
                authored_animation.is_none_or(|value| value.equipment_aim);
            let equipment_reload_supported =
                authored_animation.is_none_or(|value| value.equipment_reload);
            let melee_attack_supported = authored_animation.is_none_or(|value| {
                binding.weapon.weapon_type != WeaponType::Unarmed || value.unarmed_attack
            });
            let mut state = world
                .get::<PlayerWeaponState>(player)
                .copied()
                .unwrap_or_else(|| {
                    binding
                        .weapon
                        .firearm
                        .map(|firearm| PlayerWeaponState::loaded(firearm.tuning))
                        .unwrap_or_else(PlayerWeaponState::melee)
                });
            state.cooldown_remaining = (state.cooldown_remaining - dt).max(0.0);
            state.aiming = capabilities.aim && equipment_aim_supported && actions.aim_held;

            let mut events = Vec::<WeaponEvent>::new();

            if let Some(firearm) = binding.weapon.firearm {
                queue_weapon_obstruction_probe(world, player, fixed_tick);
                let tuning = firearm.tuning.sanitized();
                state.reserve_ammo = equipped_reserve_ammo(world, player).unwrap_or(0);

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
                            binding.instance_id,
                            state.shot_sequence,
                        ));
                    }
                }

                if capabilities.reload
                    && equipment_reload_supported
                    && combat_policy.allow_reload
                    && actions.reload_pressed
                    && state.reload_remaining <= 0.0
                    && state.ammo_in_magazine < tuning.magazine_capacity
                    && state.reserve_ammo > 0
                {
                    state.reload_remaining = tuning.reload_duration;
                    events.push(weapon_event(
                        WeaponEventKind::ReloadStarted,
                        player,
                        binding.instance_id,
                        state.shot_sequence,
                    ));
                }

                let trigger_active = match firearm.fire_mode {
                    WeaponFireMode::SemiAuto => actions.fire_primary_pressed,
                    WeaponFireMode::Automatic => actions.fire_primary_held,
                };

                let mut fire_request = None;
                if capabilities.fire
                    && equipment_ready_supported
                    && combat_policy.allow_fire
                    && trigger_active
                    && state.reload_remaining <= 0.0
                    && state.cooldown_remaining <= 0.0
                {
                    if state.ammo_in_magazine == 0 {
                        if !state.empty_latched {
                            events.push(weapon_event(
                                WeaponEventKind::Empty,
                                player,
                                binding.instance_id,
                                state.shot_sequence,
                            ));
                            state.empty_latched = true;
                        }
                    } else {
                        let shot_sequence = state.shot_sequence.wrapping_add(1);
                        if let Some((origin, direction)) = shot_origin_and_direction(
                            world,
                            player,
                            tuning,
                            state.aiming,
                            shot_sequence,
                        ) {
                            // A shot transaction only commits after an authored physical muzzle
                            // exists. Missing presentation never consumes ammo and never falls back
                            // to a camera/body origin.
                            state.ammo_in_magazine -= 1;
                            state.shot_sequence = shot_sequence;
                            state.cooldown_remaining = tuning.fire_interval;
                            state.empty_latched = false;
                            fire_request = Some((shot_sequence, state.aiming, origin, direction));
                            events.push(weapon_event(
                                WeaponEventKind::Fired,
                                player,
                                binding.instance_id,
                                shot_sequence,
                            ));
                        } else {
                            newengine_ulog_api::ulog::warn!(
                                "weapon fire rejected: shooter={} instance={} reason=physical-muzzle-unavailable",
                                player.stable_u64(),
                                binding.instance_id.0,
                            );
                        }
                    }
                } else if !actions.fire_primary_held {
                    state.empty_latched = false;
                }

                let _ = world.insert(player, state);
                persist_equipped_weapon_state(world, player);

                if let Some((shot_sequence, aiming, origin, direction)) = fire_request {
                    let pending = PendingHitscan {
                        query_seq: hitscan_query_seq(player, shot_sequence),
                        weapon_instance_id: binding.instance_id,
                        attack_kind: WeaponAttackKind::Firearm,
                        shot_sequence,
                        origin,
                        direction,
                        range: tuning.range,
                        damage: tuning.damage * combat_policy.damage_multiplier,
                    };
                    let _ = world.insert(player, pending);
                    newengine_ulog_api::ulog::info!(
                        "weapon firearm attack: shooter={} instance={} attack={} ammo_after={} origin={:?} direction={:?}",
                        player.stable_u64(),
                        binding.instance_id.0,
                        shot_sequence,
                        state.ammo_in_magazine,
                        origin,
                        direction,
                    );
                    apply_recoil(
                        world,
                        player,
                        binding.instance_id,
                        tuning,
                        aiming,
                        shot_sequence,
                    );
                }
            } else if let Some(melee) = binding.weapon.melee {
                let melee = melee.sanitized();
                let _ = world.remove::<PendingWeaponObstructionProbe>(player);
                let _ = world.remove::<WeaponObstructionState>(player);
                state.ammo_in_magazine = 0;
                state.reserve_ammo = 0;
                state.reload_remaining = 0.0;
                state.empty_latched = false;

                if capabilities.melee
                    && melee_attack_supported
                    && combat_policy.allow_melee
                    && actions.fire_primary_pressed
                    && state.cooldown_remaining <= 0.0
                {
                    state.shot_sequence = state.shot_sequence.wrapping_add(1);
                    state.cooldown_remaining = melee.attack_interval;
                    if let Some((origin, direction)) =
                        melee_origin_and_direction(world, player, melee)
                    {
                        let attack_sequence = state.shot_sequence;
                        let pending = PendingHitscan {
                            query_seq: hitscan_query_seq(player, attack_sequence),
                            weapon_instance_id: binding.instance_id,
                            attack_kind: WeaponAttackKind::Melee,
                            shot_sequence: attack_sequence,
                            origin,
                            direction,
                            range: melee.range,
                            damage: melee.damage * combat_policy.damage_multiplier,
                        };
                        let _ = world.insert(player, pending);
                        events.push(weapon_event(
                            WeaponEventKind::MeleeAttacked,
                            player,
                            binding.instance_id,
                            attack_sequence,
                        ));
                        newengine_ulog_api::ulog::info!(
                            "weapon melee attack: shooter={} instance={} attack={} type={:?} rank={} range={:.3}",
                            player.stable_u64(),
                            binding.instance_id.0,
                            attack_sequence,
                            binding.weapon.weapon_type,
                            binding.weapon.rank,
                            melee.range,
                        );
                    }
                }

                let _ = world.insert(player, state);
                persist_equipped_weapon_state(world, player);
            }

            for event in events {
                emit_weapon_event(world, event);
            }
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
    weapon_instance_id: ItemInstanceId,
    tuning: HitscanWeaponTuning,
    aiming: bool,
    shot_sequence: u64,
) {
    let tuning = tuning.sanitized();
    let ads_scale = if aiming {
        tuning.ads_recoil_multiplier
    } else {
        1.0
    };
    let pitch_noise = signed_unit(shot_sequence ^ 0x243f_6a88_85a3_08d3);
    let yaw_noise = signed_unit(shot_sequence ^ 0x1319_8a2e_0370_7344);
    let pitch_kick =
        (tuning.recoil_pitch_radians + pitch_noise * tuning.recoil_pitch_random_radians).max(0.0)
            * ads_scale;
    let yaw_kick =
        (tuning.recoil_yaw_bias_radians + yaw_noise * tuning.recoil_yaw_radians) * ads_scale;

    let previous = world.get::<WeaponRecoilRuntime>(player).copied();
    if let Some(previous) = previous.filter(|state| state.weapon_instance_id != weapon_instance_id)
    {
        if let Some(motor) = world.get_mut::<CharacterMotor>(player) {
            motor.pitch = (motor.pitch - previous.applied_pitch_radians)
                .clamp(-motor.pitch_limit, motor.pitch_limit);
            motor.yaw -= previous.applied_yaw_radians;
        }
        let _ = world.remove::<WeaponRecoilRuntime>(player);
    }

    let Some(motor) = world.get_mut::<CharacterMotor>(player) else {
        return;
    };
    // Positive pitch rotates the canonical -Z forward vector upward. Keep the immediate impulse
    // responsive, then let the recoil tracker carry a short follow-through before settling.
    let prior_pitch = motor.pitch;
    motor.pitch = (motor.pitch + pitch_kick).clamp(-motor.pitch_limit, motor.pitch_limit);
    let applied_pitch_kick = motor.pitch - prior_pitch;
    motor.yaw += yaw_kick;

    let mut recoil =
        world
            .get::<WeaponRecoilRuntime>(player)
            .copied()
            .unwrap_or(WeaponRecoilRuntime {
                weapon_instance_id,
                applied_pitch_radians: 0.0,
                applied_yaw_radians: 0.0,
                pitch_speed_radians_per_second: 0.0,
                yaw_speed_radians_per_second: 0.0,
                recovery_hz: tuning.recoil_recovery_hz,
            });
    recoil.weapon_instance_id = weapon_instance_id;
    recoil.applied_pitch_radians += applied_pitch_kick;
    recoil.applied_yaw_radians += yaw_kick;
    // Initial tracker speed produces the authored 1-2 frame follow-through seen in TLOU2 VEPR
    // fire layers instead of snapping immediately into recovery.
    recoil.pitch_speed_radians_per_second += applied_pitch_kick * tuning.recoil_recovery_hz * 1.4;
    recoil.yaw_speed_radians_per_second += yaw_kick * tuning.recoil_recovery_hz * 1.15;
    recoil.recovery_hz = tuning.recoil_recovery_hz;
    let _ = world.insert(player, recoil);
}

fn weapon_event(
    kind: WeaponEventKind,
    shooter: EntityId,
    weapon_instance_id: ItemInstanceId,
    shot_sequence: u64,
) -> WeaponEvent {
    WeaponEvent {
        kind,
        shooter,
        weapon_instance_id,
        target: None,
        shot_sequence,
        damage: 0.0,
        point: Vec3::ZERO,
        normal: Vec3::ZERO,
    }
}

fn semantic_weapon_event_id(kind: WeaponEventKind) -> &'static str {
    match kind {
        WeaponEventKind::Fired => GAMEPLAY_EVENT_WEAPON_FIRED,
        WeaponEventKind::MeleeAttacked => GAMEPLAY_EVENT_WEAPON_MELEE_ATTACKED,
        WeaponEventKind::Empty => GAMEPLAY_EVENT_WEAPON_EMPTY,
        WeaponEventKind::ReloadStarted => GAMEPLAY_EVENT_WEAPON_RELOAD_STARTED,
        WeaponEventKind::ReloadCompleted => GAMEPLAY_EVENT_WEAPON_RELOAD_COMPLETED,
        WeaponEventKind::Hit => GAMEPLAY_EVENT_WEAPON_HIT,
    }
}

#[inline]
fn vec3_payload(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

fn publish_weapon_project_event(world: &mut World, event: &WeaponEvent) {
    let binding = world
        .get::<EquippedWeaponBinding>(event.shooter)
        .copied()
        .filter(|binding| binding.instance_id == event.weapon_instance_id);
    let item = binding.map(|binding| binding.item);
    let item_name = item.and_then(|item| {
        world
            .resource::<ItemCatalog>()
            .and_then(|catalog| catalog.get(item))
            .map(|definition| definition.name.clone())
    });
    let state = world.get::<PlayerWeaponState>(event.shooter).copied();
    let muzzle = active_equipped_weapon_muzzle(world, event.shooter);
    let pending = matches!(
        event.kind,
        WeaponEventKind::Fired | WeaponEventKind::MeleeAttacked | WeaponEventKind::Hit
    )
    .then(|| world.get::<PendingHitscan>(event.shooter).copied())
    .flatten()
    .filter(|pending| {
        pending.weapon_instance_id == event.weapon_instance_id
            && pending.shot_sequence == event.shot_sequence
    });
    let attack_kind = pending.map(|pending| match pending.attack_kind {
        WeaponAttackKind::Firearm => "firearm",
        WeaponAttackKind::Melee => "melee",
    });

    let payload = serde_json::json!({
        "schema": "newengine.gameplay.weapon_event.v1",
        "version": 1,
        "weapon_instance_id": event.weapon_instance_id.0,
        "weapon_item_id": item.map(|item| item.raw()),
        "weapon": item_name,
        "shot_sequence": event.shot_sequence,
        "attack_kind": attack_kind,
        "target": event.target.map(EntityId::stable_u64),
        "damage": if event.damage > 0.0 {
            event.damage
        } else {
            pending.map(|pending| pending.damage).unwrap_or(0.0)
        },
        "point": vec3_payload(event.point),
        "normal": vec3_payload(event.normal),
        "muzzle_position": muzzle.map(|muzzle| vec3_payload(muzzle.position)),
        "muzzle_forward": muzzle.map(|muzzle| vec3_payload(muzzle.forward)),
        "shot_origin": pending.map(|pending| vec3_payload(pending.origin)),
        "shot_direction": pending.map(|pending| vec3_payload(pending.direction)),
        "range": pending.map(|pending| pending.range),
        "aiming": state.map(|state| state.aiming),
        "ammo_in_magazine": state.map(|state| state.ammo_in_magazine),
        "reserve_ammo": state.map(|state| state.reserve_ammo),
    });

    let animation_event =
        binding.and_then(|binding| match (event.kind, binding.weapon.weapon_type) {
            (WeaponEventKind::Fired, WeaponType::Firearm) => Some("character.weapon.firearm.fire"),
            (WeaponEventKind::MeleeAttacked, WeaponType::Unarmed) => {
                Some("character.weapon.unarmed.attack")
            }
            (WeaponEventKind::MeleeAttacked, WeaponType::Melee) => {
                Some("character.weapon.melee.attack")
            }
            (WeaponEventKind::ReloadStarted, WeaponType::Firearm) => {
                Some("character.weapon.firearm.reload_started")
            }
            (WeaponEventKind::ReloadCompleted, WeaponType::Firearm) => {
                Some("character.weapon.firearm.reload_completed")
            }
            _ => None,
        });
    if let Some(animation_event) = animation_event {
        if let Err(error) = emit_animation_pulse(
            world,
            event.shooter,
            "character.weapon.action",
            animation_event,
            payload.clone(),
        ) {
            newengine_ulog_api::ulog::warn!(
                "weapon animation semantic pulse rejected event='{}' shooter={} err='{}'",
                animation_event,
                event.shooter.stable_u64(),
                error,
            );
        }
    }

    if let Err(error) = emit_gameplay_event(
        world,
        semantic_weapon_event_id(event.kind),
        Some(event.shooter),
        payload,
    ) {
        newengine_ulog_api::ulog::warn!(
            "weapon semantic event rejected: event='{}' shooter={} err='{}'",
            semantic_weapon_event_id(event.kind),
            event.shooter.stable_u64(),
            error,
        );
    }
}

pub(super) fn emit_weapon_event(world: &mut World, event: WeaponEvent) {
    publish_weapon_project_event(world, &event);
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
