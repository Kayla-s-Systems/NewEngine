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
        queries.push(PhysicsQueryDto {
            seq: pending.query_seq,
            ignore_entity: Some(entity.stable_u64()),
            kind: PhysicsQueryKindDto::Ray {
                origin: vec3_to_array(pending.origin),
                dir: vec3_to_array(pending.direction),
                max_t: pending.range,
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

    let pending_shots = world
        .query::<PendingHitscan>()
        .map(|(entity, pending)| (entity, *pending))
        .collect::<Vec<_>>();
    for (shooter, pending) in pending_shots {
        consumed.insert(pending.query_seq);
        if let Some(hit) = hits.iter().find(|hit| hit.seq == pending.query_seq) {
            let hit_point = vec3_from_array(hit.position);
            crate::projectiles::clamp_weapon_shot_fx_to_hit(
                world,
                shooter,
                pending.shot_sequence,
                hit_point,
            );
            let target = key_to_entity.get(&hit.entity).copied();
            let event = FpsPolicyEvent::Hit {
                shooter: shooter.stable_u64(),
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
            let requested_damage = if decision.allow_default {
                pending.damage * decision.damage_multiplier
            } else {
                0.0
            };
            let applied_damage = target
                .and_then(|target| world.get_mut::<Health>(target))
                .map(|health| health.apply_damage(requested_damage))
                .unwrap_or(0.0);
            apply_callback_status(world, decision.status);
            emit_weapon_event(
                world,
                WeaponEvent {
                    kind: WeaponEventKind::Hit,
                    shooter,
                    target,
                    shot_sequence: pending.shot_sequence,
                    damage: applied_damage,
                    point: vec3_from_array(hit.position),
                    normal: vec3_from_array(hit.normal),
                },
            );
        }
        let _ = world.remove::<PendingHitscan>(shooter);
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
