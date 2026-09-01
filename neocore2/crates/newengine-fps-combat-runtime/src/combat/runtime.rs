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
    // NorthStar exposes both a recoil angle and a recoil-tracker speed. Model that contract as a
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

pub(super) fn recover_weapon_accuracy(world: &mut World, player: EntityId, dt: f32) {
    let Some(binding) = active_equipped_weapon_binding(world, player) else {
        let _ = world.remove::<WeaponAccuracyState>(player);
        return;
    };
    let Some(firearm) = binding.weapon.firearm else {
        let _ = world.remove::<WeaponAccuracyState>(player);
        return;
    };
    let tuning = firearm.tuning.sanitized();
    let mut state = world
        .get::<WeaponAccuracyState>(player)
        .copied()
        .filter(|state| state.weapon_instance_id == binding.instance_id)
        .unwrap_or_else(|| WeaponAccuracyState::new(binding.instance_id));
    state.time_since_shot = (state.time_since_shot + dt.max(0.0)).min(60.0);
    if state.time_since_shot >= tuning.accuracy_recovery_delay_seconds && state.bloom_radians > 0.0
    {
        let omega = tuning.accuracy_recovery_hz.max(0.05) * 2.0;
        let c = state.recovery_velocity + omega * state.bloom_radians;
        let decay = (-omega * dt.max(0.0)).exp();
        state.bloom_radians = ((state.bloom_radians + c * dt.max(0.0)) * decay)
            .clamp(0.0, tuning.recoil_accuracy_max_radians);
        state.recovery_velocity = (state.recovery_velocity - omega * c * dt.max(0.0)) * decay;
        if state.bloom_radians < 1.0e-5 && state.recovery_velocity.abs() < 1.0e-4 {
            state.bloom_radians = 0.0;
            state.recovery_velocity = 0.0;
            state.shot_count = 0;
        }
    }
    let _ = world.insert(player, state);
}

pub(super) fn kick_weapon_accuracy(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    tuning: HitscanWeaponTuning,
) {
    let tuning = tuning.sanitized();
    let mut state = world
        .get::<WeaponAccuracyState>(player)
        .copied()
        .filter(|state| state.weapon_instance_id == weapon_instance_id)
        .unwrap_or_else(|| WeaponAccuracyState::new(weapon_instance_id));
    state.bloom_radians = (state.bloom_radians + tuning.recoil_accuracy_per_shot_radians)
        .clamp(0.0, tuning.recoil_accuracy_max_radians);
    // Positive velocity delays the initial recovery and gives automatic fire a genuine accuracy
    // state instead of coupling dispersion to camera recoil.
    state.recovery_velocity +=
        tuning.recoil_accuracy_per_shot_radians * tuning.accuracy_recovery_hz * 0.35;
    state.shot_count = state.shot_count.saturating_add(1);
    state.time_since_shot = 0.0;
    let _ = world.insert(player, state);
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

pub(super) fn fire_controller_wants_shot(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    pattern: FiringPatternDefinition,
    actions: FpsActionFrame,
    dt: f32,
) -> bool {
    let pattern = pattern.sanitized();
    let mut state = world
        .get::<WeaponFireControllerState>(player)
        .copied()
        .filter(|state| state.weapon_instance_id == weapon_instance_id)
        .unwrap_or_else(|| WeaponFireControllerState::new(weapon_instance_id));
    state.pattern_cooldown_seconds = (state.pattern_cooldown_seconds - dt.max(0.0)).max(0.0);
    let release_edge = state.trigger_was_held && !actions.fire_primary_held;
    if actions.fire_primary_held {
        state.activation_seconds = (state.activation_seconds + dt.max(0.0)).min(60.0);
    } else if !matches!(pattern.kind, FiringPatternKind::Charge) {
        state.activation_seconds = 0.0;
    }

    if matches!(
        pattern.kind,
        FiringPatternKind::Burst | FiringPatternKind::ScriptedSequence
    ) && actions.fire_primary_pressed
        && state.burst_shots_remaining == 0
        && state.pattern_cooldown_seconds <= 0.0
    {
        state.bursts_remaining = pattern.bursts_min;
        state.burst_shots_remaining = pattern.shots_per_burst_min;
    }

    let wants = match pattern.kind {
        FiringPatternKind::Semi | FiringPatternKind::Pump | FiringPatternKind::BoltAction => {
            actions.fire_primary_pressed && state.pattern_cooldown_seconds <= 0.0
        }
        FiringPatternKind::Automatic => {
            actions.fire_primary_held
                && state.activation_seconds >= pattern.delay_before_firing
                && state.pattern_cooldown_seconds <= 0.0
        }
        FiringPatternKind::Burst | FiringPatternKind::ScriptedSequence => {
            state.burst_shots_remaining > 0 && state.pattern_cooldown_seconds <= 0.0
        }
        FiringPatternKind::Charge => {
            release_edge && state.activation_seconds >= pattern.delay_before_firing
        }
        FiringPatternKind::SpinUp => {
            actions.fire_primary_held
                && state.activation_seconds >= pattern.delay_before_firing
                && state.pattern_cooldown_seconds <= 0.0
        }
        FiringPatternKind::Binary => {
            (actions.fire_primary_pressed || release_edge) && state.pattern_cooldown_seconds <= 0.0
        }
    };
    if release_edge && matches!(pattern.kind, FiringPatternKind::Charge) {
        state.activation_seconds = 0.0;
    }
    state.trigger_was_held = actions.fire_primary_held;
    let _ = world.insert(player, state);
    wants
}

pub(super) fn fire_controller_commit_shot(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    pattern: FiringPatternDefinition,
) {
    let pattern = pattern.sanitized();
    let mut state = world
        .get::<WeaponFireControllerState>(player)
        .copied()
        .filter(|state| state.weapon_instance_id == weapon_instance_id)
        .unwrap_or_else(|| WeaponFireControllerState::new(weapon_instance_id));
    match pattern.kind {
        FiringPatternKind::Burst | FiringPatternKind::ScriptedSequence => {
            state.burst_shots_remaining = state.burst_shots_remaining.saturating_sub(1);
            if state.burst_shots_remaining == 0 {
                state.bursts_remaining = state.bursts_remaining.saturating_sub(1);
                if state.bursts_remaining > 0 {
                    state.burst_shots_remaining = pattern.shots_per_burst_min;
                    state.pattern_cooldown_seconds = pattern.time_between_bursts;
                } else {
                    state.pattern_cooldown_seconds = pattern.burst_cooldown;
                }
            }
        }
        FiringPatternKind::Pump | FiringPatternKind::BoltAction => {
            state.pattern_cooldown_seconds = pattern
                .burst_cooldown
                .max(pattern.time_between_bursts)
                .max(pattern.time_between_shots);
        }
        FiringPatternKind::Binary => {
            state.pattern_cooldown_seconds = pattern.time_between_shots;
        }
        FiringPatternKind::Charge => {
            state.activation_seconds = 0.0;
            state.pattern_cooldown_seconds = pattern.burst_cooldown;
        }
        FiringPatternKind::Semi | FiringPatternKind::Automatic | FiringPatternKind::SpinUp => {}
    }
    let _ = world.insert(player, state);
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
        recover_weapon_accuracy(world, player, dt);
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

                let firing_pattern = firearm.firing_pattern.sanitized();
                let trigger_active = fire_controller_wants_shot(
                    world,
                    player,
                    binding.instance_id,
                    firing_pattern,
                    actions,
                    dt,
                );

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
                        let ammo_profile = world
                            .resource::<ItemCatalog>()
                            .and_then(|catalog| catalog.get(firearm.ammo_item))
                            .and_then(|definition| definition.ammo_profile.clone());
                        if let (Some(ammo_profile), Some((origin, direction))) = (
                            ammo_profile,
                            shot_origin_and_direction(
                                world,
                                player,
                                tuning,
                                state.aiming,
                                shot_sequence,
                            ),
                        ) {
                            // A shot transaction commits only when both authored physical muzzle
                            // and authored ammo ballistics are present. Missing data never consumes
                            // ammo and never falls back to engine defaults.
                            state.ammo_in_magazine -= 1;
                            state.shot_sequence = shot_sequence;
                            state.cooldown_remaining = firing_pattern.time_between_shots;
                            state.empty_latched = false;
                            let ammo_profile = ammo_profile.sanitized();
                            let component_modifiers =
                                active_equipped_weapon_component_modifiers(world, player);
                            let effective_velocity = ammo_profile.muzzle_velocity_mps
                                * component_modifiers.muzzle_velocity_multiplier;
                            fire_request = Some((
                                shot_sequence,
                                state.aiming,
                                origin,
                                direction,
                                BallisticShotProfile {
                                    projectile_mass_kg: ammo_profile.projectile_mass_kg,
                                    muzzle_velocity_mps: effective_velocity,
                                    momentum_ns: ammo_profile.projectile_mass_kg
                                        * effective_velocity,
                                    remaining_penetration_energy_j: ammo_profile
                                        .penetration_energy_j
                                        * component_modifiers.penetration_multiplier,
                                    max_penetration_m: ammo_profile.max_penetration_m,
                                    damage_multiplier: ammo_profile.damage_multiplier
                                        * component_modifiers.damage_multiplier,
                                    impulse_multiplier: ammo_profile.impulse_multiplier,
                                    falloff_start_m: ammo_profile.falloff_start_m,
                                    falloff_end_m: ammo_profile.falloff_end_m,
                                    falloff_min_multiplier: ammo_profile.falloff_min_multiplier,
                                    component_falloff_multiplier: component_modifiers
                                        .falloff_multiplier,
                                }
                                .sanitized(),
                            ));
                            fire_controller_commit_shot(
                                world,
                                player,
                                binding.instance_id,
                                firing_pattern,
                            );
                            events.push(weapon_event(
                                WeaponEventKind::Fired,
                                player,
                                binding.instance_id,
                                shot_sequence,
                            ));
                        } else {
                            newengine_ulog_api::ulog::warn!(
                                "weapon fire rejected: shooter={} instance={} reason=authored-shot-prerequisite-unavailable (muzzle-or-ammo)",
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

                if let Some((shot_sequence, aiming, origin, direction, ballistics)) = fire_request {
                    let pending = PendingHitscan {
                        query_seq: hitscan_query_seq(player, shot_sequence),
                        weapon_instance_id: binding.instance_id,
                        attack_kind: WeaponAttackKind::Firearm,
                        shot_sequence,
                        origin,
                        direction,
                        range: tuning.range,
                        damage: tuning.damage * combat_policy.damage_multiplier,
                        ballistics,
                        bounce_count: 0,
                        max_bounces: if tuning.ricochet_enabled {
                            tuning.ricochet_max_bounces
                        } else {
                            0
                        },
                        ricochet_grazing_dot: tuning.ricochet_grazing_dot,
                        ricochet_energy_retention: tuning.ricochet_energy_retention,
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
                    kick_weapon_accuracy(world, player, binding.instance_id, tuning);
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
                            ballistics: BallisticShotProfile {
                                projectile_mass_kg: 0.0,
                                muzzle_velocity_mps: 0.0,
                                momentum_ns: 0.0,
                                remaining_penetration_energy_j: 0.0,
                                max_penetration_m: 0.0,
                                damage_multiplier: 1.0,
                                impulse_multiplier: 0.0,
                                falloff_start_m: 0.0,
                                falloff_end_m: melee.range.max(0.001),
                                falloff_min_multiplier: 1.0,
                                component_falloff_multiplier: 1.0,
                            }
                            .sanitized(),
                            bounce_count: 0,
                            max_bounces: 0,
                            ricochet_grazing_dot: 0.0,
                            ricochet_energy_retention: 0.0,
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
    let component_recoil =
        active_equipped_weapon_component_modifiers(world, player).recoil_multiplier;
    let ads_scale = (if aiming {
        tuning.ads_recoil_multiplier
    } else {
        1.0
    }) * component_recoil;
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
    // Angle impulse and tracker velocity are independent authored quantities. This preserves a
    // crisp trigger response while allowing each weapon to own how strongly recoil continues for
    // the first few frames before the critically damped recovery takes over.
    recoil.pitch_speed_radians_per_second +=
        applied_pitch_kick * tuning.recoil_recovery_hz * tuning.recoil_pitch_tracker_speed_scale;
    recoil.yaw_speed_radians_per_second +=
        yaw_kick * tuning.recoil_recovery_hz * tuning.recoil_yaw_tracker_speed_scale;
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
    let hit_surface = event.target.and_then(|target| {
        world
            .get::<PhysicsSurface>(target)
            .map(|surface| surface.id.clone())
    });
    let incidence_dot = pending
        .filter(|pending| {
            event.kind == WeaponEventKind::Hit && pending.attack_kind == WeaponAttackKind::Firearm
        })
        .map(|pending| ricochet_incidence_dot(pending.direction, event.normal));
    let ricochet_material = event.target.and_then(|target| {
        world
            .get::<BallisticMaterialResponse>(target)
            .copied()
            .map(BallisticMaterialResponse::sanitized)
    });
    let ricochet = pending
        .filter(|pending| {
            event.kind == WeaponEventKind::Hit && pending.attack_kind == WeaponAttackKind::Firearm
        })
        .zip(ricochet_material)
        .is_some_and(|(pending, material)| {
            ballistic_material_allows_ricochet(
                material,
                pending.direction,
                event.normal,
                pending.bounce_count,
                pending.max_bounces,
            )
        });
    let ricochet_direction = pending
        .filter(|_| ricochet)
        .map(|pending| {
            let incoming = pending.direction.normalize_or_zero();
            let normal = event.normal.normalize_or_zero();
            (incoming - normal * (2.0 * incoming.dot(normal))).normalize_or_zero()
        })
        .filter(|direction| direction.length_squared() > 1.0e-8);
    let ricochet_range = pending.filter(|_| ricochet).map(|pending| {
        let hit_distance = (event.point - pending.origin)
            .length()
            .clamp(0.0, pending.range);
        let retention = ricochet_material
            .map(|material| material.ricochet_energy_retention)
            .unwrap_or(0.0);
        (pending.range - hit_distance).max(0.0) * retention
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
        "bounce_count": pending.map(|pending| pending.bounce_count),
        "surface": hit_surface,
        "incidence_dot": incidence_dot,
        "ricochet": ricochet,
        "ricochet_direction": ricochet_direction.map(vec3_payload),
        "ricochet_range": ricochet_range,
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
