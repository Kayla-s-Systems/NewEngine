#![forbid(unsafe_op_in_unsafe_fn)]

//! Reusable FPS combat, interaction targeting and hitscan mechanics.
//! Weapon definitions and mission outcomes remain project-authored.

mod game_data;
mod script_commands;

use std::collections::{BTreeMap, BTreeSet};

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    active_equipped_weapon_binding, active_equipped_weapon_muzzle, consume_equipped_ammo,
    emit_animation_pulse, emit_gameplay_event, equipped_reserve_ammo,
    persist_equipped_weapon_state, sync_equipped_weapon_runtime, try_collect_item_pickup,
    EquippedWeaponBinding, Health, HitscanWeaponTuning, Interactable, InteractionEvent,
    InteractionEventBus, ItemCatalog, ItemInstanceId, ItemPickup, MeleeWeaponTuning,
    PendingHitscan, PendingInteraction, PlayerAuthoredAnimationCapabilities, PlayerCommandFrame,
    PlayerController, PlayerInteractionTuning, PlayerStanceState, PlayerWeaponState,
    WeaponAttackKind, WeaponEvent, WeaponEventBus, WeaponEventKind, WeaponFireMode,
    WeaponObstructionState, WeaponType, GAMEPLAY_EVENT_WEAPON_EMPTY, GAMEPLAY_EVENT_WEAPON_FIRED,
    GAMEPLAY_EVENT_WEAPON_HIT, GAMEPLAY_EVENT_WEAPON_MELEE_ATTACKED,
    GAMEPLAY_EVENT_WEAPON_RELOAD_COMPLETED, GAMEPLAY_EVENT_WEAPON_RELOAD_STARTED,
};
#[cfg(test)]
use newengine_gameplay_fps_api::action as fps_action;
use newengine_gameplay_fps_api::{
    FpsActionFrame, FpsGameplayPolicyProvider, FpsGameplayPolicySnapshot, FpsPolicyDecision,
    FpsPolicyEvent,
};
use newengine_gameplay_script_runtime::GameplayCommandExecutor;
use newengine_math::{avalanche_u64, EulerRot, Quat, Vec3};
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto, PhysicsQueryKindDto};
use newengine_sim::CharacterMotor;
use newengine_transform::Transform;

#[cfg(test)]
use newengine_engine_runtime::gameplay::EquippedWeaponMuzzle;

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingFocusedItemInteraction {
    target: EntityId,
    point: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingWeaponObstructionProbe {
    query_seq: u64,
    origin: Vec3,
    direction: Vec3,
    muzzle_distance: f32,
    muzzle_position: Vec3,
}

/// Chooses the nearest enabled inventory pickup inside the same interaction radius used by
/// the player. World-item pickup UX is intentionally proximity based: a long/thin weapon
/// lying at the player's feet must not require pixel-perfect ray intersection to advertise
/// or collect it. Generic non-item interactables remain ray targeted.
pub fn focused_item_pickup(world: &World, player: EntityId) -> Option<EntityId> {
    let player_transform = world.get::<Transform>(player)?;
    let range = world
        .get::<PlayerInteractionTuning>(player)
        .copied()
        .unwrap_or_default()
        .range
        .clamp(0.1, 100.0);
    let range_sq = range * range;

    world
        .query::<ItemPickup>()
        .filter_map(|(entity, pickup)| {
            if !pickup.enabled || pickup.quantity == 0 {
                return None;
            }
            let interactable = world.get::<Interactable>(entity)?;
            if !interactable.enabled {
                return None;
            }
            let target = world.get::<Transform>(entity)?;
            let delta = target.position - player_transform.position;
            let distance_sq = delta.length_squared();
            if !distance_sq.is_finite() || distance_sq > range_sq {
                return None;
            }
            Some((entity, distance_sq))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(entity, _)| entity)
}

#[path = "combat/queries.rs"]
mod queries;
#[path = "combat/runtime.rs"]
mod runtime;
#[path = "combat/targeting.rs"]
mod targeting;

pub use queries::{collect_combat_queries, resolve_combat_queries};
pub use runtime::step_player_combat;

#[cfg(test)]
use runtime::{apply_recoil, recover_weapon_recoil};
use runtime::{emit_interaction_event, emit_weapon_event};
use targeting::{
    hitscan_query_seq, interaction_query_seq, interaction_ray, melee_origin_and_direction,
    queue_weapon_obstruction_probe, shot_origin_and_direction, signed_unit,
};

#[cfg(test)]
include!("combat/tests.rs");
