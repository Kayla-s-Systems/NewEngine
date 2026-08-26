use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    drop_item, equip_item_instance, first_player, select_equipment_slot, use_item, EquipmentSlot,
    EquippedWeaponBinding, GameplayInputCapture, GameplayUiFrameOutput, GameplayUiProvider,
    GameplayWorld, Interactable, ItemCatalog, ItemInstanceId, ItemKind, ItemPickup,
    PlayerCommandFrame, PlayerController, PlayerInventory, PlayerWeaponState,
};
#[cfg(test)]
use newengine_gameplay_fps_api::action as fps_action;
use newengine_gameplay_fps_api::{FpsActionFrame, FpsDemoState, FpsPlayableCharacterPolicy};
use newengine_ui_api::{UiEventDispatchFrame, UiNodeEventTrigger, UiStatePatch};

use crate::combat::focused_item_pickup;

mod character_variants;
mod commands;
mod helpers;
mod interaction;
mod provider;
mod publish;
mod state;
#[cfg(test)]
mod tests;

pub(crate) use character_variants::reconcile_existing_player_assignments_with_policy;
pub(crate) use commands::step_inventory_commands;
pub use provider::FpsInventoryHudProvider;
pub(crate) use state::character_select_is_open;

use character_variants::{
    availability_label, playable_character_variants, selected_variant, variant_from_action,
    PlayableCharacterSelection,
};
use helpers::*;
#[cfg(test)]
use interaction::{
    activate_inventory_instance, drop_instance_quantity, equip_dragged_instance, reorder_inventory,
};
use interaction::{apply_inventory_ui_actions, select_playable_character};
use publish::publish_inventory_hud_state;
#[cfg(test)]
use state::inventory_hud_is_visible;
use state::{
    ensure_inventory_hud_state, inventory_hud_is_open, inventory_slot_count, InventoryDragState,
    InventoryHudState, CHARACTER_UI_ACTION_TOGGLE, INVENTORY_HUD_CONTRACT,
    INVENTORY_HUD_SURFACE_ID, INVENTORY_UI_ACTION_DROP, INVENTORY_UI_ACTION_EQUIPMENT,
    INVENTORY_UI_ACTION_HOTBAR, INVENTORY_UI_ACTION_SLOT, INVENTORY_UI_ACTION_TOGGLE,
};
