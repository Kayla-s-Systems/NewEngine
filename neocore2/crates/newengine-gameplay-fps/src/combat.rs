use std::collections::{BTreeMap, BTreeSet};

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    consume_equipped_ammo, equipped_reserve_ammo, persist_equipped_weapon_state,
    sync_equipped_weapon_runtime, try_collect_item_pickup, EquippedWeaponBinding, Health,
    HitscanWeaponTuning, Interactable, InteractionEvent, InteractionEventBus, PendingHitscan,
    PendingInteraction, PlayerCommandFrame, PlayerController, PlayerInteractionTuning,
    PlayerStanceState, PlayerWeaponState, WeaponEvent, WeaponEventBus, WeaponEventKind,
};
#[cfg(test)]
use newengine_gameplay_fps_api::action as fps_action;
use newengine_gameplay_fps_api::{
    FpsActionFrame, FpsGameplayPolicyProvider, FpsGameplayPolicySnapshot, FpsPolicyDecision,
    FpsPolicyEvent,
};
use newengine_gameplay_script_runtime::GameplayCommandExecutor;
use newengine_math::{avalanche_u64, Vec3};
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto, PhysicsQueryKindDto};
use newengine_sim::CharacterMotor;
use newengine_transform::Transform;

#[path = "combat/queries.rs"]
mod queries;
#[path = "combat/runtime.rs"]
mod runtime;
#[path = "combat/targeting.rs"]
mod targeting;

pub use queries::{collect_combat_queries, resolve_combat_queries};
pub use runtime::step_player_combat;

#[cfg(test)]
use runtime::apply_recoil;
use runtime::{emit_interaction_event, emit_weapon_event};
use targeting::{
    hitscan_query_seq, interaction_query_seq, interaction_ray, shot_origin_and_direction,
    signed_unit,
};

#[cfg(test)]
include!("combat/tests.rs");
