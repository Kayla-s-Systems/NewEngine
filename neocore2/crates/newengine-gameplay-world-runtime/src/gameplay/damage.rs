use std::collections::BTreeMap;

use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_sim::MotorInput;

use super::{
    active_equipped_weapon_binding, drop_item_instance, emit_gameplay_event,
    reconcile_character_life_state, sync_equipped_weapon_runtime, CharacterControlState,
    CharacterExertionState, Health, PlayerController, PlayerInventory,
    GAMEPLAY_EVENT_CHARACTER_CORPSE, GAMEPLAY_EVENT_CHARACTER_DAMAGED,
    GAMEPLAY_EVENT_CHARACTER_DEATH_PRESENTATION_REQUESTED, GAMEPLAY_EVENT_CHARACTER_DIED,
    GAMEPLAY_EVENT_CHARACTER_HIT_REACTION, GAMEPLAY_EVENT_CHARACTER_INJURED,
    GAMEPLAY_EVENT_CHARACTER_INJURY_RECOVERED,
};

#[path = "damage/types.rs"]
mod types;
pub use types::*;

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[path = "damage/character_response.rs"]
mod character_response;
use character_response::{apply_character_death_transition, publish_character_hit_reaction};
pub use character_response::{
    mark_character_corpse, reconcile_character_injury_state, update_character_damage_states,
};

pub fn resolve_weapon_impact(world: &mut World, impact: WeaponImpact) -> Option<DamageResolution> {
    // Fail closed: only authored damage receivers participate in the damage domain.
    let receiver = world
        .get::<DamageReceiver>(impact.target)
        .copied()?
        .sanitized();
    let zone = world
        .get::<DamageHitZoneMap>(impact.target)
        .and_then(|zones| zones.by_subshape.get(&impact.subshape_id))
        .cloned()
        .map(DamageHitZone::sanitized);

    let falloff = finite_or(impact.falloff_multiplier, 1.0).clamp(0.0, 20.0);
    let zone_damage = zone
        .as_ref()
        .map(|zone| zone.damage_multiplier)
        .unwrap_or(1.0);
    let armor = 1.0
        - (1.0 - receiver.armor_absorption)
            * (1.0
                - zone
                    .as_ref()
                    .map(|zone| zone.armor_absorption)
                    .unwrap_or(0.0));
    let requested_damage = (finite_or(impact.base_damage, 0.0).max(0.0)
        * receiver.damage_multiplier
        * zone_damage
        * falloff
        * (1.0 - armor))
        .max(0.0);
    let applied_damage = world
        .get_mut::<Health>(impact.target)
        .map(|health| health.apply_damage(requested_damage))
        .unwrap_or(0.0);

    let impulse_scale = receiver.impulse_multiplier
        * zone
            .as_ref()
            .map(|zone| zone.impulse_multiplier)
            .unwrap_or(1.0)
        * finite_or(impact.ammo_impulse_multiplier, 1.0).max(0.0);
    let impulse = impact.direction.normalize_or_zero()
        * finite_or(impact.momentum_ns, 0.0).max(0.0)
        * impulse_scale;
    if impulse.length_squared() > 1.0e-10 {
        let _ = world.insert(
            impact.target,
            PendingPhysicsImpulse {
                sequence: impact.sequence,
                impulse,
                point: impact.point,
            },
        );
    }

    let mut reaction = CharacterHitReactionKind::None;
    let mut injured = false;
    let mut lethal = false;
    if receiver.kind == DamageReceiverKind::Character && applied_damage > 0.0 {
        let health = world
            .get::<Health>(impact.target)
            .copied()
            .unwrap_or_default();
        let _ = emit_gameplay_event(
            world,
            GAMEPLAY_EVENT_CHARACTER_DAMAGED,
            Some(impact.target),
            serde_json::json!({
                "instigator": impact.source.stable_u64(),
                "sequence": impact.sequence,
                "hit_zone": zone.as_ref().map(|zone| zone.id.as_str()),
                "requested_damage": requested_damage,
                "applied_damage": applied_damage,
                "health_current": health.current,
                "health_maximum": health.maximum,
                "health_normalized": health.normalized(),
                "impulse": [impulse.x, impulse.y, impulse.z],
            }),
        );
        lethal = reconcile_character_life_state(world, impact.target);
        if lethal {
            let _ = emit_gameplay_event(
                world,
                GAMEPLAY_EVENT_CHARACTER_DIED,
                Some(impact.target),
                serde_json::json!({
                    "instigator": impact.source.stable_u64(),
                    "sequence": impact.sequence,
                    "hit_zone": zone.as_ref().map(|zone| zone.id.as_str()),
                    "health_current": health.current,
                    "health_maximum": health.maximum,
                    "impulse": [impulse.x, impulse.y, impulse.z],
                }),
            );
            let _ = apply_character_death_transition(world, impact, zone.as_ref(), impulse);
        } else {
            injured = reconcile_character_injury_state(world, impact.target);
            reaction = publish_character_hit_reaction(
                world,
                impact,
                zone.as_ref(),
                impulse,
                applied_damage,
                health,
            );
        }
    }

    Some(DamageResolution {
        receiver_kind: receiver.kind,
        hit_zone: zone.map(|zone| zone.id),
        requested_damage,
        applied_damage,
        impulse,
        reaction,
        injured,
        lethal,
    })
}

#[cfg(test)]
#[path = "damage/tests.rs"]
mod tests;
