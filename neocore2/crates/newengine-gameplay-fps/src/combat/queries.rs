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
    for (_, pending) in world.query::<PendingHitscan>() {
        queries.push(PhysicsQueryDto {
            seq: pending.query_seq,
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
            kind: PhysicsQueryKindDto::Ray {
                origin: vec3_to_array(pending.origin),
                dir: vec3_to_array(pending.direction),
                max_t: pending.range,
            },
        });
    }
    queries
}

/// Resolves all pending hitscan/interaction requests and returns query sequences consumed by this
/// subsystem. Pending requests are always removed, including misses, so a render or fixed tick can
/// never replay the same shot/interaction.
pub fn resolve_combat_queries(
    world: &mut World,
    fixed_tick: u64,
    hits: &[PhysicsQueryHitDto],
    key_to_entity: &BTreeMap<u64, EntityId>,
) -> BTreeSet<u64> {
    let mut consumed = BTreeSet::new();
    let pending_shots = world
        .query::<PendingHitscan>()
        .map(|(entity, pending)| (entity, *pending))
        .collect::<Vec<_>>();
    for (shooter, pending) in pending_shots {
        consumed.insert(pending.query_seq);
        if let Some(hit) = hits.iter().find(|hit| hit.seq == pending.query_seq) {
            let target = key_to_entity.get(&hit.entity).copied();
            let applied_damage = target
                .and_then(|target| world.get_mut::<Health>(target))
                .map(|health| health.apply_damage(pending.damage))
                .unwrap_or(0.0);
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
                        let _ = try_collect_item_pickup(world, player, target);
                    }
                }
            }
        }
        let _ = world.remove::<PendingInteraction>(player);
    }
    consumed
}
