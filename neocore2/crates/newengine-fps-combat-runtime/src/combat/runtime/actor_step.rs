pub fn step_actor_combat(world: &mut World, dt: f32, fixed_tick: u64) {
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
    let mut actors = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .filter_map(|entity| {
            let commands = world.get::<PlayerCommandFrame>(entity)?;
            Some((
                entity,
                FpsActionFrame::from_commands(&commands.actions),
                commands.source_frame,
            ))
        })
        .collect::<Vec<_>>();
    actors.extend(
        world
            .query::<CombatActuationState>()
            .filter(|(entity, _)| world.get::<PlayerController>(*entity).is_none())
            .map(|(entity, frame)| {
                (
                    entity,
                    FpsActionFrame {
                        fire_primary_pressed: frame.trigger_pressed,
                        fire_primary_held: frame.trigger_held,
                        aim_held: frame.aim,
                        reload_pressed: frame.reload_pressed,
                        ..FpsActionFrame::default()
                    },
                    frame.source_frame,
                )
            }),
    );
    actors.sort_by_key(|(entity, _, _)| entity.stable_u64());
    actors.dedup_by_key(|(entity, _, _)| entity.stable_u64());

    for (player, actions, source_frame) in actors {
        // Inventory/equipment is the only authority for firearm availability. Player input and
        // AI actuation converge here; neither path can synthesize a weapon state or damage event.
        sync_equipped_weapon_runtime(world, player);
        recover_weapon_recoil(world, player, dt);
        recover_weapon_accuracy(world, player, dt);

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

            let mut events = Vec::<WeaponEvent>::with_capacity(6);
            ensure_weapon_action_runtime(world, player, binding.instance_id);
            step_transient_weapon_action(world, player, dt);

            if let Some(firearm) = binding.weapon.firearm {
                queue_weapon_obstruction_probe(world, player, fixed_tick);
                let tuning = firearm.tuning.sanitized();
                let profiles = firearm.profiles.sanitized();
                let resolved_stats = resolved_weapon_stats(world, player);
                state.reserve_ammo = equipped_reserve_ammo(world, player).unwrap_or(0);

                let reload_active = world
                    .get::<WeaponActionRuntime>(player)
                    .copied()
                    .is_some_and(|action| {
                        action.weapon_instance_id == binding.instance_id
                            && action.action == WeaponActionKind::Reloading
                    });
                if reload_active {
                    let reload_step = step_reload_action(
                        world,
                        player,
                        binding.instance_id,
                        profiles.handling.reload_timeline,
                        dt,
                    );
                    if reload_step.magazine_detached {
                        events.push(weapon_event(
                            WeaponEventKind::ReloadMagazineDetached,
                            player,
                            binding.instance_id,
                            state.shot_sequence,
                        ));
                    }
                    if reload_step.ammo_committed {
                        let needed = tuning
                            .magazine_capacity
                            .saturating_sub(state.ammo_in_magazine);
                        let moved = consume_equipped_ammo(world, player, needed);
                        state.ammo_in_magazine += moved;
                        state.reserve_ammo = equipped_reserve_ammo(world, player).unwrap_or(0);
                        events.push(weapon_event(
                            WeaponEventKind::ReloadAmmoCommitted,
                            player,
                            binding.instance_id,
                            state.shot_sequence,
                        ));
                    }
                    if reload_step.magazine_inserted {
                        events.push(weapon_event(
                            WeaponEventKind::ReloadMagazineInserted,
                            player,
                            binding.instance_id,
                            state.shot_sequence,
                        ));
                    }
                    if reload_step.chambered {
                        events.push(weapon_event(
                            WeaponEventKind::ReloadChambered,
                            player,
                            binding.instance_id,
                            state.shot_sequence,
                        ));
                    }
                    if reload_step.completed {
                        state.reload_remaining = 0.0;
                        events.push(weapon_event(
                            WeaponEventKind::ReloadCompleted,
                            player,
                            binding.instance_id,
                            state.shot_sequence,
                        ));
                    } else if let Some(action) = world.get::<WeaponActionRuntime>(player).copied() {
                        state.reload_remaining =
                            (action.duration_seconds - action.elapsed_seconds).max(0.0);
                    }
                } else {
                    // Compatibility projection only. WeaponActionRuntime is the authoritative
                    // action state; stale per-instance countdowns are cancelled on weapon switch.
                    state.reload_remaining = 0.0;
                }

                let reloading = world
                    .get::<WeaponActionRuntime>(player)
                    .copied()
                    .is_some_and(|action| {
                        action.weapon_instance_id == binding.instance_id
                            && action.action == WeaponActionKind::Reloading
                    });
                if capabilities.reload
                    && equipment_reload_supported
                    && combat_policy.allow_reload
                    && actions.reload_pressed
                    && !reloading
                    && state.ammo_in_magazine < tuning.magazine_capacity
                    && state.reserve_ammo > 0
                {
                    let (timing_source, authored_duration) = reload_timing_source(
                        world,
                        player,
                        binding.instance_id,
                        profiles.handling.reload_duration_seconds,
                    );
                    let reload_duration =
                        authored_duration * resolved_stats.reload_duration_multiplier;
                    begin_reload_action(
                        world,
                        player,
                        binding.instance_id,
                        reload_duration,
                        timing_source,
                    );
                    state.reload_remaining = reload_duration;
                    events.push(weapon_event(
                        WeaponEventKind::ReloadStarted,
                        player,
                        binding.instance_id,
                        state.shot_sequence,
                    ));
                }

                let reloading = world
                    .get::<WeaponActionRuntime>(player)
                    .copied()
                    .is_some_and(|action| {
                        action.weapon_instance_id == binding.instance_id
                            && action.action == WeaponActionKind::Reloading
                    });

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
                    && !reloading
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
                            shot_origin_and_direction_with_profiles(
                                world,
                                player,
                                tuning,
                                profiles,
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
                            let effective_velocity = ammo_profile.muzzle_velocity_mps
                                * resolved_stats.muzzle_velocity_multiplier;
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
                                        * resolved_stats.penetration_multiplier,
                                    max_penetration_m: ammo_profile.max_penetration_m,
                                    damage_multiplier: ammo_profile.damage_multiplier
                                        * resolved_stats.damage_multiplier,
                                    impulse_multiplier: ammo_profile.impulse_multiplier,
                                    falloff_start_m: ammo_profile.falloff_start_m,
                                    falloff_end_m: ammo_profile.falloff_end_m,
                                    falloff_min_multiplier: ammo_profile.falloff_min_multiplier,
                                    component_falloff_multiplier: resolved_stats.falloff_multiplier,
                                }
                                .sanitized(),
                            ));
                            fire_controller_commit_shot(
                                world,
                                player,
                                binding.instance_id,
                                firing_pattern,
                            );
                            let (action_kind, action_duration) = firing_action(firing_pattern);
                            mark_transient_weapon_action(
                                world,
                                player,
                                binding.instance_id,
                                action_kind,
                                action_duration,
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
                    apply_recoil_with_profile(
                        world,
                        player,
                        binding.instance_id,
                        profiles.recoil,
                        aiming,
                        shot_sequence,
                    );
                    kick_weapon_accuracy_with_profile(
                        world,
                        player,
                        binding.instance_id,
                        profiles.spread,
                        aiming,
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
                        mark_transient_weapon_action(
                            world,
                            player,
                            binding.instance_id,
                            WeaponActionKind::Melee,
                            melee.attack_interval,
                        );
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
