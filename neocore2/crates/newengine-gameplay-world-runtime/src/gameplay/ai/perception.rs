use super::*;

#[inline]
fn perception_point(world: &World, entity: EntityId) -> Option<Vec3> {
    let transform = world.get::<Transform>(entity)?;
    let body = world
        .get::<CharacterBody>(entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let crouched = world
        .get::<PlayerStanceState>(entity)
        .is_some_and(|stance| matches!(stance.current, PlayerStanceKind::Crouched));
    let eye_height = if crouched {
        body.crouched_eye_height
    } else {
        body.standing_eye_height
    };
    Some(transform.position + Vec3::Y * eye_height)
}

#[inline]
pub(super) fn controller_is_operational(world: &World, entity: EntityId) -> bool {
    world
        .get::<AIController>(entity)
        .copied()
        .unwrap_or_default()
        .sanitized()
        .enabled
        && world
            .get::<CharacterLifeState>(entity)
            .is_none_or(|state| state.alive())
        && world
            .get::<CharacterControlState>(entity)
            .is_none_or(|state| state.enabled)
        && world
            .get::<Health>(entity)
            .is_none_or(|health| health.alive())
}

pub(super) fn clear_ai_runtime_state(world: &mut World, entity: EntityId) {
    let _ = world.remove::<AIPerceptionProbe>(entity);
    if let Some(perception) = world.get_mut::<PerceptionState>(entity) {
        perception.candidate_target = None;
        perception.visible_target = None;
        perception.candidate_distance = 0.0;
    }
    if let Some(memory) = world.get_mut::<TargetMemory>(entity) {
        if memory.target.is_some() || memory.visible {
            memory.target = None;
            memory.visible = false;
            memory.seconds_since_seen = 0.0;
            memory.revision = memory.revision.wrapping_add(1);
        }
    }
    set_combat_intent(world, entity, CombatIntentKind::Idle, None, Vec3::ZERO);
}

pub(super) fn set_combat_intent(
    world: &mut World,
    entity: EntityId,
    kind: CombatIntentKind,
    target: Option<EntityId>,
    target_position: Vec3,
) {
    let previous = world
        .get::<CombatIntent>(entity)
        .copied()
        .unwrap_or_default();
    let changed = previous.kind != kind
        || previous.target != target
        || (previous.target_position - target_position).length_squared() > 1.0e-8;
    let next = CombatIntent {
        kind,
        target,
        target_position,
        revision: previous.revision.wrapping_add(u64::from(changed)),
    };
    let _ = world.insert(entity, next);
}

fn candidate_is_targetable(world: &World, observer: EntityId, candidate: EntityId) -> bool {
    if observer == candidate {
        return false;
    }
    if world
        .get::<CharacterLifeState>(candidate)
        .is_some_and(|state| !state.alive())
    {
        return false;
    }
    if world
        .get::<Health>(candidate)
        .is_some_and(|health| !health.alive())
    {
        return false;
    }
    let Some(observer_team) = world.get::<CombatTeam>(observer).copied() else {
        return false;
    };
    let Some(candidate_team) = world.get::<CombatTeam>(candidate).copied() else {
        return false;
    };
    observer_team.hostile_to(candidate_team)
}

fn choose_perception_candidate(world: &World, observer: EntityId) -> Option<(EntityId, f32)> {
    let tuning = world
        .get::<PerceptionTuning>(observer)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let observer_transform = world.get::<Transform>(observer)?;
    let origin = perception_point(world, observer)?;
    let forward = (observer_transform.rotation * Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    if forward.length_squared() <= 1.0e-8 {
        return None;
    }
    let half_fov_cos = (0.5 * tuning.field_of_view_degrees.to_radians()).cos();
    let mut candidates = Vec::new();
    for candidate in world.iter_entities() {
        if !candidate_is_targetable(world, observer, candidate) {
            continue;
        }
        let Some(target_point) = perception_point(world, candidate) else {
            continue;
        };
        let delta = target_point - origin;
        let distance_sq = delta.length_squared();
        if !distance_sq.is_finite() || distance_sq <= 1.0e-8 {
            continue;
        }
        let distance = distance_sq.sqrt();
        if distance > tuning.sight_range {
            continue;
        }
        let direction = delta / distance;
        if tuning.field_of_view_degrees < 359.999 && forward.dot(direction) < half_fov_cos {
            continue;
        }
        candidates.push((candidate, distance));
    }
    candidates.sort_by(|(left_id, left_distance), (right_id, right_distance)| {
        left_distance
            .partial_cmp(right_distance)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left_id.stable_u64().cmp(&right_id.stable_u64()))
    });
    candidates.into_iter().next()
}

#[inline]
fn perception_probe_seq(observer: EntityId, target: EntityId) -> u64 {
    let mut value =
        observer.stable_u64() ^ target.stable_u64().rotate_left(29) ^ 0xa17e_0000_0000_0001;
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}

pub fn prepare_ai_perception(world: &mut World, dt: f32) {
    let dt = finite_non_negative(dt, 0.0).min(0.25);
    let agents = world
        .query::<AIController>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    for agent in agents {
        if !controller_is_operational(world, agent) {
            clear_ai_runtime_state(world, agent);
            continue;
        }

        let tuning = world
            .get::<PerceptionTuning>(agent)
            .copied()
            .unwrap_or_default()
            .sanitized();
        if let Some(memory) = world.get_mut::<TargetMemory>(agent) {
            if memory.target.is_some() && !memory.visible {
                memory.seconds_since_seen =
                    (memory.seconds_since_seen + dt).min(tuning.memory_seconds);
                if memory.seconds_since_seen + 1.0e-6 >= tuning.memory_seconds {
                    memory.target = None;
                    memory.seconds_since_seen = tuning.memory_seconds;
                    memory.revision = memory.revision.wrapping_add(1);
                }
            }
        } else {
            let _ = world.insert(agent, TargetMemory::default());
        }

        let Some((target, distance)) = choose_perception_candidate(world, agent) else {
            let _ = world.remove::<AIPerceptionProbe>(agent);
            if let Some(perception) = world.get_mut::<PerceptionState>(agent) {
                let changed =
                    perception.candidate_target.is_some() || perception.visible_target.is_some();
                perception.candidate_target = None;
                perception.visible_target = None;
                perception.candidate_distance = 0.0;
                perception.observation_revision = perception
                    .observation_revision
                    .wrapping_add(u64::from(changed));
            } else {
                let _ = world.insert(agent, PerceptionState::default());
            }
            if let Some(memory) = world.get_mut::<TargetMemory>(agent) {
                if memory.target.is_some() && memory.visible {
                    memory.visible = false;
                    memory.seconds_since_seen =
                        (memory.seconds_since_seen + dt).min(tuning.memory_seconds);
                    if memory.seconds_since_seen + 1.0e-6 >= tuning.memory_seconds {
                        memory.target = None;
                    }
                    memory.revision = memory.revision.wrapping_add(1);
                }
            }
            continue;
        };

        let Some(origin) = perception_point(world, agent) else {
            continue;
        };
        let Some(target_point) = perception_point(world, target) else {
            continue;
        };
        let delta = target_point - origin;
        let max_distance = delta.length().max(0.001);
        let direction = delta / max_distance;
        let seq = perception_probe_seq(agent, target);
        let _ = world.insert(
            agent,
            AIPerceptionProbe {
                seq,
                target,
                origin,
                direction,
                max_distance,
                sample_dt: dt,
            },
        );
        let previous = world
            .get::<PerceptionState>(agent)
            .copied()
            .unwrap_or_default();
        let changed = previous.candidate_target != Some(target)
            || (previous.candidate_distance - distance).abs() > 1.0e-4;
        let _ = world.insert(
            agent,
            PerceptionState {
                candidate_target: Some(target),
                visible_target: previous.visible_target,
                candidate_distance: distance,
                observation_revision: previous
                    .observation_revision
                    .wrapping_add(u64::from(changed)),
            },
        );
    }
}

pub fn collect_ai_perception_queries(world: &World) -> Vec<PhysicsQueryDto> {
    let mut probes = world
        .query::<AIPerceptionProbe>()
        .filter_map(|(entity, probe)| {
            controller_is_operational(world, entity).then_some((entity, *probe))
        })
        .collect::<Vec<_>>();
    probes.sort_by_key(|(entity, _)| entity.stable_u64());
    probes
        .into_iter()
        .map(|(observer, probe)| PhysicsQueryDto {
            seq: probe.seq,
            ignore_entity: Some(observer.stable_u64()),
            kind: PhysicsQueryKindDto::Ray {
                origin: [probe.origin.x, probe.origin.y, probe.origin.z],
                dir: [probe.direction.x, probe.direction.y, probe.direction.z],
                max_t: probe.max_distance + 0.05,
            },
        })
        .collect()
}

pub fn resolve_ai_perception_query_hits(
    world: &mut World,
    _fixed_tick: u64,
    hits: &[PhysicsQueryHitDto],
    key_to_entity: &BTreeMap<u64, EntityId>,
) -> BTreeSet<u64> {
    let agents = world
        .query::<AIPerceptionProbe>()
        .map(|(entity, probe)| (entity, *probe))
        .collect::<Vec<_>>();
    let mut consumed = BTreeSet::new();
    for (agent, probe) in agents {
        consumed.insert(probe.seq);
        if !controller_is_operational(world, agent) {
            clear_ai_runtime_state(world, agent);
            continue;
        }
        let visible = hits
            .iter()
            .filter(|hit| hit.seq == probe.seq)
            .min_by(|left, right| {
                left.distance
                    .partial_cmp(&right.distance)
                    .unwrap_or(Ordering::Equal)
            })
            .and_then(|hit| key_to_entity.get(&hit.entity).copied())
            == Some(probe.target);
        let previous = world
            .get::<PerceptionState>(agent)
            .copied()
            .unwrap_or_default();
        let changed = previous.visible_target != visible.then_some(probe.target);
        let _ = world.insert(
            agent,
            PerceptionState {
                candidate_target: Some(probe.target),
                visible_target: visible.then_some(probe.target),
                candidate_distance: previous.candidate_distance,
                observation_revision: previous
                    .observation_revision
                    .wrapping_add(u64::from(changed)),
            },
        );

        let previous_memory = world
            .get::<TargetMemory>(agent)
            .copied()
            .unwrap_or_default();
        if visible {
            let target_position = world
                .get::<Transform>(probe.target)
                .map(|transform| transform.position)
                .unwrap_or(previous_memory.last_known_position);
            let memory_changed = previous_memory.target != Some(probe.target)
                || !previous_memory.visible
                || (previous_memory.last_known_position - target_position).length_squared()
                    > 1.0e-8;
            let _ = world.insert(
                agent,
                TargetMemory {
                    target: Some(probe.target),
                    visible: true,
                    last_known_position: target_position,
                    seconds_since_seen: 0.0,
                    revision: previous_memory
                        .revision
                        .wrapping_add(u64::from(memory_changed)),
                },
            );
        } else if previous_memory.target.is_some() {
            let mut memory = previous_memory;
            if memory.visible {
                let tuning = world
                    .get::<PerceptionTuning>(agent)
                    .copied()
                    .unwrap_or_default()
                    .sanitized();
                memory.visible = false;
                memory.seconds_since_seen =
                    (memory.seconds_since_seen + probe.sample_dt).min(tuning.memory_seconds);
                if memory.seconds_since_seen + 1.0e-6 >= tuning.memory_seconds {
                    memory.target = None;
                }
                memory.revision = memory.revision.wrapping_add(1);
            }
            let _ = world.insert(agent, memory);
        }
    }
    consumed
}
