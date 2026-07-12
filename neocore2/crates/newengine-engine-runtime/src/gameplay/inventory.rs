use std::collections::{BTreeMap, VecDeque};

use newengine_ecs::{EntityId, World};
use newengine_math::{avalanche_u64 as mix64, fnv1a_64 as stable_hash64, Vec3};
use newengine_primitives::{builtins as primitive_builtins, Primitive, PrimitiveId};
use newengine_scene::components::Name;
use newengine_sim::{AngularVelocity, Velocity};
use newengine_transform::{set_parent, Transform};

use super::combat::{Health, HitscanWeaponTuning, Interactable, PlayerWeaponState};
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
    EquipmentSlot, ItemDefinition, ItemId, ItemInstanceId, ItemKind, ItemUseEffect,
    WeaponItemDefinition, WorldItemDefinition, WorldItemPresentation, WorldItemRuntime,
    WorldItemVisualPart,
};
pub use loadouts::{InventoryLoadout, InventoryLoadoutCatalog, InventoryLoadoutEntry};
pub use operations::{
    apply_loadout, default_fps_loadout_id, default_medkit_item_id, default_rifle_ammo_id,
    default_rifle_item_id, drain_inventory_events, ensure_default_item_catalog,
    ensure_player_inventory, give_default_fps_loadout, give_item, inventory_quantity, remove_item,
};
pub use storage::{
    EquippedWeaponBinding, InventoryEntry, InventoryEvent, InventoryEventBus, InventoryEventKind,
    InventoryMutation, ItemPickup, PlayerInventory,
};

pub use inventory_equipment::{
    consume_equipped_ammo, equip_first_item, equip_item_instance, equipped_reserve_ammo,
    persist_equipped_weapon_state, select_equipment_slot, sync_equipped_weapon_runtime,
    unequip_slot, use_item,
};
pub(crate) use inventory_world::try_collect_item_pickup;
pub use inventory_world::{
    drop_item, spawn_item_pickup, spawn_persistent_item_pickup, step_world_items,
};

pub const DEFAULT_RIFLE_ITEM_NAME: &str = "weapon.rifle.standard";
pub const DEFAULT_RIFLE_AMMO_NAME: &str = "ammo.rifle.standard";
pub const DEFAULT_MEDKIT_ITEM_NAME: &str = "consumable.medkit.standard";
pub const DEFAULT_FPS_LOADOUT_NAME: &str = "loadout.fps.default";

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

#[cfg(test)]
mod inventory_tests;
