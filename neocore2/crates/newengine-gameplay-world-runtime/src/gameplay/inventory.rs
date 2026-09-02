use std::collections::{BTreeMap, VecDeque};

use newengine_ecs::{EntityId, World};
use newengine_math::{avalanche_u64 as mix64, fnv1a_64 as stable_hash64, Vec3};
use newengine_primitives::{builtins as primitive_builtins, Primitive, PrimitiveId};
use newengine_scene::components::Name;
use newengine_sim::{AngularVelocity, Velocity};
use newengine_transform::{set_parent, Transform};

use super::combat::{HitscanWeaponTuning, Interactable, PlayerWeaponState};
use super::Health;
use super::{
    attach_scene_object_core, CollisionShapeDesc, DisplayMode, DisplayVisibility, GameplayActor,
    PhysicsBodyDesc, PhysicsSurface,
};

mod catalog;
mod definitions;
mod inventory_equipment;
mod inventory_world;
mod loadouts;
mod operations;
mod storage;

pub use catalog::ItemCatalog;
pub use definitions::{
    AmmoDefinition, AmmoProjectileType, EquipmentSlot, FirearmWeaponDefinition,
    FiringPatternDefinition, FiringPatternKind, ItemDefinition, ItemId, ItemInstanceId, ItemKind,
    ItemUseEffect, MeleeWeaponTuning, ResolvedWeaponStats, WeaponAdsProfile,
    WeaponAnimationDefinition, WeaponAudioAction, WeaponAudioDefinition, WeaponCapabilities,
    WeaponCasingDefinition, WeaponComponentDefinition, WeaponComponentGraphDefinition,
    WeaponComponentInstance, WeaponComponentModifiers, WeaponComponentPointDefinition,
    WeaponFireMode, WeaponHandlingProfile, WeaponItemDefinition, WeaponPresentationDefinition,
    WeaponRecoilProfile, WeaponRecoilStateProfile, WeaponReloadTimelineProfile,
    WeaponReloadTopology, WeaponRuntimeProfiles, WeaponSpreadDistribution, WeaponSpreadProfile,
    WeaponSpreadStateProfile, WeaponStatId, WeaponStatModifier, WeaponStatModifierOp,
    WeaponStatModifierStack, WeaponSwayProfile, WeaponType, WeaponVfxDefinition,
    WorldItemDefinition, WorldItemPresentation, WorldItemRuntime, WorldItemVisualPart,
    SHARED_UNARMED_WEAPON_ITEM_NAME,
};
pub use loadouts::{InventoryLoadout, InventoryLoadoutCatalog, InventoryLoadoutEntry};
pub use operations::{
    apply_loadout, drain_inventory_events, ensure_inventory_runtime, ensure_player_inventory,
    give_item, inventory_capacity_state, inventory_quantity, merge_inventory_stacks, remove_item,
    reorder_inventory_instance, split_inventory_stack,
};
pub use storage::{
    EquippedWeaponBinding, EquippedWeaponEntity, EquippedWeaponMuzzle, InventoryCapacityState,
    InventoryEntry, InventoryEvent, InventoryEventBus, InventoryEventKind, InventoryMutation,
    ItemPickup, PlayerInventory, WeaponEntityRuntime, WeaponEntitySockets, WeaponSocketPose,
};

pub use inventory_equipment::{
    active_equipped_weapon_aiming, active_equipped_weapon_binding, active_equipped_weapon_can_aim,
    active_equipped_weapon_can_fire, active_equipped_weapon_can_melee,
    active_equipped_weapon_component_modifiers, active_equipped_weapon_component_overrides,
    active_equipped_weapon_component_stat_modifiers, active_equipped_weapon_muzzle,
    consume_equipped_ammo, equip_first_item, equip_item_instance, equipped_reserve_ammo,
    install_weapon_component, persist_equipped_weapon_state, play_equipped_weapon_audio,
    play_weapon_item_audio, preload_weapon_audio_definition, remove_weapon_component,
    select_equipment_slot, select_highest_ranked_equipped_weapon, sync_equipped_weapon_runtime,
    unequip_slot, use_item, use_item_instance,
};
pub use inventory_world::try_collect_item_pickup;
pub use inventory_world::{
    drop_item, drop_item_instance, spawn_item_pickup, spawn_persistent_item_pickup,
    step_world_items,
};

fn normalize_item_name(raw: &str) -> Option<String> {
    let mut normalized = String::with_capacity(raw.len());
    let mut separator = false;
    for character in raw.trim().chars() {
        let character = character.to_ascii_lowercase();
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '/') {
            normalized.push(character);
            separator = false;
        } else if !separator && !normalized.is_empty() {
            normalized.push('-');
            separator = true;
        }
    }
    while normalized.ends_with('-') {
        normalized.pop();
    }
    (!normalized.is_empty()).then_some(normalized)
}

#[inline]
fn sanitize_positive_vec3(mut value: [f32; 3], minimum: f32, maximum: f32) -> [f32; 3] {
    for component in &mut value {
        *component = if component.is_finite() {
            component.abs().clamp(minimum, maximum)
        } else {
            minimum
        };
    }
    value
}

fn sanitize_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}
