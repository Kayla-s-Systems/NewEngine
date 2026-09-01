use crate::script_commands::execute_policy_commands;

use super::*;

fn vec3_to_array(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

#[inline]
fn vec3_from_array(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

pub fn collect_combat_queries(world: &World) -> Vec<PhysicsQueryDto> {
    let mut queries = Vec::new();
    for (entity, pending) in world.query::<PendingHitscan>() {
        let kind = match pending.attack_kind {
            WeaponAttackKind::Firearm => PhysicsQueryKindDto::BallisticRay {
                origin: vec3_to_array(pending.origin),
                dir: vec3_to_array(pending.direction),
                max_t: pending.range,
                max_hits: 32,
                collide_back_faces: true,
            },
            WeaponAttackKind::Melee => PhysicsQueryKindDto::Ray {
                origin: vec3_to_array(pending.origin),
                dir: vec3_to_array(pending.direction),
                max_t: pending.range,
            },
        };
        queries.push(PhysicsQueryDto {
            seq: pending.query_seq,
            ignore_entity: Some(entity.stable_u64()),
            kind,
        });
    }
    for (entity, pending) in world.query::<PendingWeaponObstructionProbe>() {
        queries.push(PhysicsQueryDto {
            seq: pending.query_seq,
            ignore_entity: Some(entity.stable_u64()),
            kind: PhysicsQueryKindDto::Ray {
                origin: vec3_to_array(pending.origin),
                dir: vec3_to_array(pending.direction),
                max_t: pending.muzzle_distance,
            },
        });
    }
    for (_, pending) in world.query::<PendingInteraction>() {
        queries.push(PhysicsQueryDto {
            seq: pending.query_seq,
            ignore_entity: None,
            kind: PhysicsQueryKindDto::Ray {
                origin: vec3_to_array(pending.origin),
                dir: vec3_to_array(pending.direction),
                max_t: pending.range,
            },
        });
    }
    queries
}

/// Resolves pending physics requests. The query execution and ECS mutations remain
/// Rust mechanisms; Lua receives immutable DTOs and may gate/parameterize default actions.
pub fn resolve_combat_queries(
    world: &mut World,
    fixed_tick: u64,
    hits: &[PhysicsQueryHitDto],
    key_to_entity: &BTreeMap<u64, EntityId>,
    policy_provider: &dyn FpsGameplayPolicyProvider,
    command_executor: &GameplayCommandExecutor,
) -> BTreeSet<u64> {
    let policy = world
        .resource::<FpsGameplayPolicySnapshot>()
        .cloned()
        .unwrap_or_default();
    let mut consumed = BTreeSet::new();

    let obstruction_probes = world
        .query::<PendingWeaponObstructionProbe>()
        .map(|(player, pending)| (player, *pending))
        .collect::<Vec<_>>();
    for (player, pending) in obstruction_probes {
        consumed.insert(pending.query_seq);
        let state = if let Some(hit) = hits.iter().find(|hit| hit.seq == pending.query_seq) {
            let hit_distance = if hit.distance.is_finite() {
                hit.distance.clamp(0.0, pending.muzzle_distance)
            } else {
                pending.muzzle_distance
            };
            // Leave a small safety shell on the player's side of the contact plane. The alpha is
            // proportional to how much of the authored barrel would have crossed the obstacle.
            let overhang = (pending.muzzle_distance - hit_distance).max(0.0);
            let blocked = overhang > 0.015;
            let alpha = if blocked {
                (overhang / 0.28).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let safe_t = (hit_distance - 0.025).max(0.0);
            WeaponObstructionState {
                blocked,
                alpha,
                hit_position: vec3_from_array(hit.position),
                hit_normal: vec3_from_array(hit.normal),
                safe_muzzle_position: pending.origin + pending.direction * safe_t,
                fixed_tick,
            }
        } else {
            WeaponObstructionState::clear(pending.muzzle_position, fixed_tick)
        };
        let _ = world.insert(player, state);
        let _ = world.remove::<PendingWeaponObstructionProbe>(player);
    }

    let pending_shots = world
        .query::<PendingHitscan>()
        .map(|(entity, pending)| (entity, *pending))
        .collect::<Vec<_>>();
    for (shooter, pending) in pending_shots {
        consumed.insert(pending.query_seq);
        let mut ricochet_trace = None;
        let mut shot_hits = hits
            .iter()
            .filter(|hit| hit.seq == pending.query_seq)
            .copied()
            .collect::<Vec<_>>();
        shot_hits.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then_with(|| a.hit_index.cmp(&b.hit_index))
        });

        if pending.attack_kind == WeaponAttackKind::Melee {
            if let Some(hit) = shot_hits.first().copied() {
                let target = key_to_entity.get(&hit.entity).copied();
                let point = vec3_from_array(hit.position);
                let normal = vec3_from_array(hit.normal).normalize_or_zero();
                let event = FpsPolicyEvent::Hit {
                    shooter: shooter.stable_u64(),
                    weapon_instance_id: pending.weapon_instance_id.0,
                    target: target.map(EntityId::stable_u64),
                    shot_sequence: pending.shot_sequence,
                    base_damage: pending.damage,
                    fixed_tick,
                    point: hit.position,
                    normal: hit.normal,
                };
                let mut decision = invoke_policy_event_fail_closed(
                    policy_provider,
                    &policy.callbacks.hit,
                    &event,
                    "hit",
                );
                if let Err(error) =
                    execute_policy_commands(world, command_executor, &decision.commands, "hit")
                {
                    decision.allow_default = false;
                    decision.status = Some(format!("Gameplay command transaction failed: {error}"));
                }
                let applied_damage = if decision.allow_default {
                    target
                        .map(|target| {
                            resolve_weapon_impact(
                                world,
                                WeaponImpact {
                                    sequence: pending.query_seq,
                                    source: shooter,
                                    target,
                                    base_damage: pending.damage * decision.damage_multiplier,
                                    point,
                                    normal,
                                    direction: pending.direction,
                                    distance: hit.distance,
                                    range: pending.range,
                                    subshape_id: hit.subshape_id,
                                    momentum_ns: 0.0,
                                    ammo_impulse_multiplier: 0.0,
                                    falloff_multiplier: 1.0,
                                },
                            )
                            .map(|resolution| resolution.applied_damage)
                            .unwrap_or(0.0)
                        })
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                apply_callback_status(world, decision.status);
                emit_weapon_event(
                    world,
                    WeaponEvent {
                        kind: WeaponEventKind::Hit,
                        shooter,
                        weapon_instance_id: pending.weapon_instance_id,
                        target,
                        shot_sequence: pending.shot_sequence,
                        damage: applied_damage,
                        point,
                        normal,
                    },
                );
            }
        } else {
            let mut energy_j = pending.ballistics.remaining_penetration_energy_j.max(0.0);
            let mut momentum_ns = pending.ballistics.momentum_ns.max(0.0);
            let mut index = 0usize;
            while index < shot_hits.len() {
                let hit = shot_hits[index];
                if hit.back_face {
                    index += 1;
                    continue;
                }
                let target = key_to_entity.get(&hit.entity).copied();
                let point = vec3_from_array(hit.position);
                let normal = vec3_from_array(hit.normal).normalize_or_zero();
                let exit_index = shot_hits
                    .iter()
                    .enumerate()
                    .skip(index + 1)
                    .find(|(_, candidate)| candidate.entity == hit.entity && candidate.back_face)
                    .map(|(candidate_index, _)| candidate_index);
                let thickness_m = exit_index
                    .map(|exit_index| (shot_hits[exit_index].distance - hit.distance).max(0.0));
                let material = target
                    .and_then(|target| world.get::<BallisticMaterialResponse>(target))
                    .copied()
                    .map(BallisticMaterialResponse::sanitized);
                let penetration_cost_j = match (material, thickness_m) {
                    (Some(material), Some(thickness))
                        if thickness <= pending.ballistics.max_penetration_m =>
                    {
                        Some(material.penetration_cost_j(thickness))
                    }
                    _ => None,
                };
                let penetrated = penetration_cost_j
                    .is_some_and(|cost| cost.is_finite() && energy_j > cost + 1.0);
                let transfer = material
                    .map(|material| material.damage_transfer_multiplier)
                    .unwrap_or(1.0);

                let impact_base_damage =
                    pending.damage * pending.ballistics.damage_multiplier * transfer;
                let event = FpsPolicyEvent::Hit {
                    shooter: shooter.stable_u64(),
                    weapon_instance_id: pending.weapon_instance_id.0,
                    target: target.map(EntityId::stable_u64),
                    shot_sequence: pending.shot_sequence,
                    base_damage: impact_base_damage,
                    fixed_tick,
                    point: hit.position,
                    normal: hit.normal,
                };
                let mut decision = invoke_policy_event_fail_closed(
                    policy_provider,
                    &policy.callbacks.hit,
                    &event,
                    "hit",
                );
                if let Err(error) =
                    execute_policy_commands(world, command_executor, &decision.commands, "hit")
                {
                    decision.allow_default = false;
                    decision.status = Some(format!("Gameplay command transaction failed: {error}"));
                }
                let applied_damage = if decision.allow_default {
                    target
                        .map(|target| {
                            resolve_weapon_impact(
                                world,
                                WeaponImpact {
                                    sequence: pending.query_seq ^ u64::from(hit.hit_index),
                                    source: shooter,
                                    target,
                                    base_damage: impact_base_damage * decision.damage_multiplier,
                                    point,
                                    normal,
                                    direction: pending.direction,
                                    distance: hit.distance,
                                    range: pending.range,
                                    subshape_id: hit.subshape_id,
                                    momentum_ns,
                                    ammo_impulse_multiplier: pending.ballistics.impulse_multiplier
                                        * material
                                            .map(|material| material.impulse_transfer_multiplier)
                                            .unwrap_or(1.0),
                                    falloff_multiplier: pending
                                        .ballistics
                                        .falloff_multiplier_at(hit.distance),
                                },
                            )
                            .map(|resolution| resolution.applied_damage)
                            .unwrap_or(0.0)
                        })
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                apply_callback_status(world, decision.status);
                emit_weapon_event(
                    world,
                    WeaponEvent {
                        kind: WeaponEventKind::Hit,
                        shooter,
                        weapon_instance_id: pending.weapon_instance_id,
                        target,
                        shot_sequence: pending.shot_sequence,
                        damage: applied_damage,
                        point,
                        normal,
                    },
                );

                if penetrated {
                    let cost = penetration_cost_j.unwrap_or(energy_j);
                    let before = energy_j.max(1.0);
                    energy_j = (energy_j - cost).max(0.0);
                    momentum_ns *= (energy_j / before).sqrt().clamp(0.0, 1.0);
                    if let Some(exit_index) = exit_index {
                        let exit_hit = shot_hits[exit_index];
                        let surface = target
                            .and_then(|target| world.get::<PhysicsSurface>(target))
                            .map(|surface| surface.id.clone());
                        let _ = emit_gameplay_event(
                            world,
                            GAMEPLAY_EVENT_WEAPON_PENETRATED,
                            Some(shooter),
                            serde_json::json!({
                                "schema": "newengine.gameplay.weapon_penetration_event.v1",
                                "version": 1,
                                "weapon_instance_id": pending.weapon_instance_id.0,
                                "shot_sequence": pending.shot_sequence,
                                "entry_point": hit.position,
                                "entry_normal": hit.normal,
                                "exit_point": exit_hit.position,
                                "exit_normal": exit_hit.normal,
                                "shot_direction": vec3_to_array(pending.direction),
                                "thickness_m": thickness_m.unwrap_or(0.0),
                                "remaining_energy_j": energy_j,
                                "remaining_momentum_ns": momentum_ns,
                                "surface": surface,
                            }),
                        );
                    }
                    index = exit_index.unwrap_or(index) + 1;
                    continue;
                }

                let ricochet_material = material.filter(|material| {
                    ballistic_material_allows_ricochet(
                        *material,
                        pending.direction,
                        normal,
                        pending.bounce_count,
                        pending.max_bounces,
                    )
                });
                if let Some(ricochet_material) = ricochet_material {
                    let retention = ricochet_material.ricochet_energy_retention;
                    let incoming = pending.direction.normalize_or_zero();
                    let reflected =
                        (incoming - normal * (2.0 * incoming.dot(normal))).normalize_or_zero();
                    let remaining = (pending.range - hit.distance.max(0.0)).max(0.0) * retention;
                    if reflected.length_squared() > 1.0e-8 && remaining > 0.25 {
                        let bounce_count = pending.bounce_count.saturating_add(1);
                        let mut ballistics = pending.ballistics;
                        ballistics.remaining_penetration_energy_j = energy_j * retention;
                        ballistics.momentum_ns = momentum_ns * retention.sqrt();
                        ricochet_trace = Some(PendingHitscan {
                            query_seq: hitscan_bounce_query_seq(
                                shooter,
                                pending.shot_sequence,
                                bounce_count,
                            ),
                            weapon_instance_id: pending.weapon_instance_id,
                            attack_kind: WeaponAttackKind::Firearm,
                            shot_sequence: pending.shot_sequence,
                            origin: point + normal * 0.006 + reflected * 0.012,
                            direction: reflected,
                            range: remaining,
                            damage: pending.damage * retention,
                            ballistics: ballistics.sanitized(),
                            bounce_count,
                            max_bounces: pending.max_bounces,
                            ricochet_grazing_dot: ricochet_material.ricochet_max_incidence_dot,
                            ricochet_energy_retention: retention,
                        });
                    }
                }
                break;
            }
        }
        let _ = world.remove::<PendingHitscan>(shooter);
        if let Some(next) = ricochet_trace {
            let _ = world.insert(shooter, next);
        }
    }

    let focused_item_interactions = world
        .query::<PendingFocusedItemInteraction>()
        .map(|(player, pending)| (player, *pending))
        .collect::<Vec<_>>();
    for (player, pending) in focused_item_interactions {
        if world.exists(pending.target) {
            if let Some(interactable) = world.get::<Interactable>(pending.target).cloned() {
                if interactable.enabled {
                    let event = FpsPolicyEvent::Interaction {
                        player: player.stable_u64(),
                        target: pending.target.stable_u64(),
                        prompt: interactable.prompt.clone(),
                        fixed_tick,
                        point: vec3_to_array(pending.point),
                    };
                    let mut decision = invoke_policy_event_fail_closed(
                        policy_provider,
                        &policy.callbacks.interaction,
                        &event,
                        "interaction",
                    );
                    if let Err(error) = execute_policy_commands(
                        world,
                        command_executor,
                        &decision.commands,
                        "interaction",
                    ) {
                        decision.allow_default = false;
                        decision.status =
                            Some(format!("Gameplay command transaction failed: {error}"));
                    }
                    apply_callback_status(world, decision.status.clone());
                    if decision.allow_default {
                        emit_interaction_event(
                            world,
                            InteractionEvent {
                                player,
                                target: pending.target,
                                prompt: interactable.prompt,
                                fixed_tick,
                                point: pending.point,
                            },
                        );
                        if decision.collect_item.unwrap_or(true) {
                            let _ = try_collect_item_pickup(world, player, pending.target);
                        }
                    }
                }
            }
        }
        let _ = world.remove::<PendingFocusedItemInteraction>(player);
    }

    let pending_interactions = world
        .query::<PendingInteraction>()
        .map(|(entity, pending)| (entity, *pending))
        .collect::<Vec<_>>();
    for (player, pending) in pending_interactions {
        consumed.insert(pending.query_seq);
        if let Some(hit) = hits.iter().find(|hit| hit.seq == pending.query_seq) {
            if let Some(target) = key_to_entity.get(&hit.entity).copied() {
                if let Some(interactable) = world.get::<Interactable>(target).cloned() {
                    if interactable.enabled {
                        let event = FpsPolicyEvent::Interaction {
                            player: player.stable_u64(),
                            target: target.stable_u64(),
                            prompt: interactable.prompt.clone(),
                            fixed_tick,
                            point: hit.position,
                        };
                        let mut decision = invoke_policy_event_fail_closed(
                            policy_provider,
                            &policy.callbacks.interaction,
                            &event,
                            "interaction",
                        );
                        if let Err(error) = execute_policy_commands(
                            world,
                            command_executor,
                            &decision.commands,
                            "interaction",
                        ) {
                            decision.allow_default = false;
                            decision.status =
                                Some(format!("Gameplay command transaction failed: {error}"));
                        }
                        apply_callback_status(world, decision.status.clone());
                        if decision.allow_default {
                            emit_interaction_event(
                                world,
                                InteractionEvent {
                                    player,
                                    target,
                                    prompt: interactable.prompt,
                                    fixed_tick,
                                    point: vec3_from_array(hit.position),
                                },
                            );
                            if decision.collect_item.unwrap_or(true) {
                                let _ = try_collect_item_pickup(world, player, target);
                            }
                        }
                    }
                }
            }
        }
        let _ = world.remove::<PendingInteraction>(player);
    }
    consumed
}

fn invoke_policy_event_fail_closed(
    policy_provider: &dyn FpsGameplayPolicyProvider,
    export: &str,
    event: &FpsPolicyEvent,
    label: &str,
) -> FpsPolicyDecision {
    if export.trim().is_empty() {
        return FpsPolicyDecision::default();
    }
    match policy_provider.invoke_event(export, event) {
        Ok(decision) => decision,
        Err(error) => {
            newengine_ulog_api::ulog::error!(
                "fps Lua {} callback failed export='{}' err='{}'; policy='fail closed: default mutation denied'",
                label,
                export,
                error
            );
            FpsPolicyDecision {
                allow_default: false,
                status: Some(format!("Gameplay script error: {error}")),
                ..FpsPolicyDecision::default()
            }
        }
    }
}

fn apply_callback_status(world: &mut World, status: Option<String>) {
    let Some(status) = status else {
        return;
    };
    if let Some(state) = world.resource_mut::<newengine_gameplay_fps_api::FpsDemoState>() {
        state.status = status;
    }
}
