use newengine_ecs::{EntityId, World};
use newengine_ui_api::{UiEventDispatchFrame, UiNodeEventTrigger, UiStatePatch};

use super::inventory::{
    drop_item, equip_item_instance, select_equipment_slot, use_item, EquipmentSlot,
    EquippedWeaponBinding, ItemCatalog, ItemInstanceId, ItemKind, PlayerInventory,
};
use super::{first_player, FpsDemoState, PlayerCommandFrame, PlayerController, PlayerWeaponState};

mod commands;
mod helpers;
mod interaction;
mod publish;
mod state;
#[cfg(test)]
mod tests;

pub use commands::step_inventory_commands;
pub use interaction::apply_inventory_ui_actions;
pub use publish::publish_inventory_hud_state;
pub use state::{
    ensure_inventory_hud_state, inventory_hud_is_open, inventory_hud_is_visible,
    InventoryDragState, InventoryHudState,
};

use helpers::*;
#[cfg(test)]
use interaction::{
    activate_inventory_instance, drop_instance_quantity, equip_dragged_instance, reorder_inventory,
};
use state::{
    INVENTORY_HUD_CONTRACT, INVENTORY_HUD_SURFACE_ID, INVENTORY_SLOT_COUNT,
    INVENTORY_UI_ACTION_DROP, INVENTORY_UI_ACTION_EQUIPMENT, INVENTORY_UI_ACTION_HOTBAR,
    INVENTORY_UI_ACTION_SLOT, INVENTORY_UI_ACTION_TOGGLE,
};
