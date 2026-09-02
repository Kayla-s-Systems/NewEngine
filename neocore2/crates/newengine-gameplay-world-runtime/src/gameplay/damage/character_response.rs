use super::*;

pub fn reconcile_character_injury_state(world: &mut World, entity: EntityId) -> bool {
    let health = world.get::<Health>(entity).copied().unwrap_or_default();
    let life_alive = world
        .get::<crate::gameplay::CharacterLifeState>(entity)
        .is_none_or(|state| state.alive());
    let tuning = world
        .get::<CharacterDamageResponseTuning>(entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let should_be_injured = life_alive
        && health.alive()
        && health.normalized() <= tuning.injured_health_fraction + 1.0e-6;
    let previous = world
        .get::<CharacterInjuryState>(entity)
        .copied()
        .unwrap_or_default();
    if previous.injured == should_be_injured {
        if world.get::<CharacterInjuryState>(entity).is_none() {
            let _ = world.insert(entity, previous);
        }
        return should_be_injured;
    }

    let next = CharacterInjuryState {
        injured: should_be_injured,
        revision: previous.revision.wrapping_add(1),
    };
    let _ = world.insert(entity, next);
    let event = if should_be_injured {
        GAMEPLAY_EVENT_CHARACTER_INJURED
    } else {
        GAMEPLAY_EVENT_CHARACTER_INJURY_RECOVERED
    };
    let _ = emit_gameplay_event(
        world,
        event,
        Some(entity),
        serde_json::json!({
            "health_current": health.current,
            "health_maximum": health.maximum,
            "health_normalized": health.normalized(),
            "injured": should_be_injured,
            "revision": next.revision,
        }),
    );
    should_be_injured
}

pub fn update_character_damage_states(world: &mut World, dt: f32) {
    let dt = finite_or(dt, 0.0).clamp(0.0, 0.25);
    if dt > 0.0 {
        let reactions = world
            .query::<CharacterHitReactionState>()
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>();
        for entity in reactions {
            if let Some(reaction) = world.get_mut::<CharacterHitReactionState>(entity) {
                reaction.remaining_seconds = (reaction.remaining_seconds - dt).max(0.0);
                if reaction.remaining_seconds <= 1.0e-6 {
                    reaction.remaining_seconds = 0.0;
                    reaction.kind = CharacterHitReactionKind::None;
                }
            }
        }
    }

    let characters = world
        .query2_ids::<Health, DamageReceiver>()
        .filter(|entity| {
            world
                .get::<DamageReceiver>(*entity)
                .is_some_and(|receiver| receiver.kind == DamageReceiverKind::Character)
        })
        .collect::<Vec<_>>();
    for entity in characters {
        let _ = reconcile_character_injury_state(world, entity);
    }
}

pub fn mark_character_corpse(world: &mut World, entity: EntityId) -> bool {
    let Some(mut death) = world.get::<CharacterDeathTransitionState>(entity).cloned() else {
        return false;
    };
    if death.phase == CharacterDeathPhase::Corpse {
        return false;
    }
    death.phase = CharacterDeathPhase::Corpse;
    death.revision = death.revision.wrapping_add(1);
    let revision = death.revision;
    let _ = world.insert(entity, death);
    let _ = emit_gameplay_event(
        world,
        GAMEPLAY_EVENT_CHARACTER_CORPSE,
        Some(entity),
        serde_json::json!({"revision": revision}),
    );
    true
}

fn select_character_hit_reaction(
    tuning: CharacterDamageResponseTuning,
    applied_damage: f32,
    health_maximum: f32,
    impulse: Vec3,
) -> CharacterHitReactionKind {
    if applied_damage <= 1.0e-6 {
        return CharacterHitReactionKind::None;
    }
    let damage_fraction = if health_maximum <= 1.0e-6 {
        0.0
    } else {
        (applied_damage / health_maximum).clamp(0.0, 1.0)
    };
    if damage_fraction + 1.0e-6 >= tuning.stagger_damage_fraction
        || impulse.length() + 1.0e-6 >= tuning.stagger_impulse_threshold
    {
        CharacterHitReactionKind::Stagger
    } else {
        CharacterHitReactionKind::Flinch
    }
}

pub(super) fn publish_character_hit_reaction(
    world: &mut World,
    impact: WeaponImpact,
    zone: Option<&DamageHitZone>,
    impulse: Vec3,
    applied_damage: f32,
    health: Health,
) -> CharacterHitReactionKind {
    let tuning = world
        .get::<CharacterDamageResponseTuning>(impact.target)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let kind = select_character_hit_reaction(tuning, applied_damage, health.maximum, impulse);
    if kind == CharacterHitReactionKind::None {
        return kind;
    }
    let previous_revision = world
        .get::<CharacterHitReactionState>(impact.target)
        .map(|state| state.revision)
        .unwrap_or(0);
    let remaining_seconds = match kind {
        CharacterHitReactionKind::None => 0.0,
        CharacterHitReactionKind::Flinch => tuning.flinch_duration_seconds,
        CharacterHitReactionKind::Stagger => tuning.stagger_duration_seconds,
    };
    let state = CharacterHitReactionState {
        kind,
        remaining_seconds,
        sequence: impact.sequence,
        source: impact.source.stable_u64(),
        hit_zone: zone.map(|zone| zone.id.clone()),
        point: impact.point,
        impulse,
        applied_damage,
        health_fraction: health.normalized(),
        revision: previous_revision.wrapping_add(1),
    };
    let revision = state.revision;
    let _ = world.insert(impact.target, state);
    let _ = emit_gameplay_event(
        world,
        GAMEPLAY_EVENT_CHARACTER_HIT_REACTION,
        Some(impact.target),
        serde_json::json!({
            "instigator": impact.source.stable_u64(),
            "sequence": impact.sequence,
            "kind": kind.as_str(),
            "hit_zone": zone.map(|zone| zone.id.as_str()),
            "applied_damage": applied_damage,
            "health_normalized": health.normalized(),
            "impulse": [impulse.x, impulse.y, impulse.z],
            "duration_seconds": remaining_seconds,
            "revision": revision,
        }),
    );
    kind
}

pub(super) fn apply_character_death_transition(
    world: &mut World,
    impact: WeaponImpact,
    zone: Option<&DamageHitZone>,
    impulse: Vec3,
) -> CharacterDeathTransitionState {
    if let Some(control) = world.get_mut::<CharacterControlState>(impact.target) {
        control.enabled = false;
    } else {
        let _ = world.insert(impact.target, CharacterControlState::disabled());
    }
    if let Some(controller) = world.get_mut::<PlayerController>(impact.target) {
        controller.enabled = false;
    }
    if let Some(input) = world.get_mut::<MotorInput>(impact.target) {
        *input = MotorInput::default();
    }
    if let Some(exertion) = world.get_mut::<CharacterExertionState>(impact.target) {
        exertion.sprinting = false;
    }

    let policy = world
        .get::<CharacterDeathPolicy>(impact.target)
        .copied()
        .unwrap_or_default();
    let dropped_weapon_entity = if policy.drop_active_weapon {
        active_equipped_weapon_binding(world, impact.target)
            .filter(|binding| !binding.is_unarmed())
            .and_then(|binding| {
                drop_item_instance(world, impact.target, binding.instance_id, 1).ok()
            })
            .map(EntityId::stable_u64)
    } else {
        None
    };
    // Manual drops are allowed to select another equipped weapon. Death is different: no
    // successor weapon may become active after the lethal transition, even if inventory still
    // contains other equipped slots. Presentation is forced back to the neutral/unarmed route.
    if policy.drop_active_weapon && dropped_weapon_entity.is_some() {
        if let Some(inventory) = world.get_mut::<PlayerInventory>(impact.target) {
            inventory.active_slot = None;
        }
        sync_equipped_weapon_runtime(world, impact.target);
    }

    let previous_revision = world
        .get::<CharacterDeathTransitionState>(impact.target)
        .map(|state| state.revision)
        .unwrap_or(0);
    let state = CharacterDeathTransitionState {
        phase: CharacterDeathPhase::TransitionRequested,
        sequence: impact.sequence,
        source: impact.source.stable_u64(),
        hit_zone: zone.map(|zone| zone.id.clone()),
        point: impact.point,
        impulse,
        dropped_weapon_entity,
        presentation: policy.presentation,
        revision: previous_revision.wrapping_add(1),
    };
    let _ = world.insert(impact.target, state.clone());
    let previous_injury = world
        .get::<CharacterInjuryState>(impact.target)
        .copied()
        .unwrap_or_default();
    let _ = world.insert(
        impact.target,
        CharacterInjuryState {
            injured: false,
            revision: previous_injury
                .revision
                .wrapping_add(u64::from(previous_injury.injured)),
        },
    );
    let _ = emit_gameplay_event(
        world,
        GAMEPLAY_EVENT_CHARACTER_DEATH_PRESENTATION_REQUESTED,
        Some(impact.target),
        serde_json::json!({
            "instigator": impact.source.stable_u64(),
            "sequence": impact.sequence,
            "hit_zone": zone.map(|zone| zone.id.as_str()),
            "presentation": policy.presentation.as_str(),
            "impulse": [impulse.x, impulse.y, impulse.z],
            "dropped_weapon_entity": dropped_weapon_entity,
            "revision": state.revision,
        }),
    );
    state
}
