#![forbid(unsafe_op_in_unsafe_fn)]

//! Optional FPS inventory/character-selection presentation provider.
//! Simulation mechanics and project content are external.

mod character_menu_policy;
mod game_data;

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    drop_item, equip_first_item, equip_item_instance, first_player, give_item,
    select_equipment_slot, sync_equipped_weapon_runtime, use_item, EquipmentSlot,
    EquippedWeaponBinding, GameplayInputCapture, GameplayUiFrameOutput, GameplayUiProvider,
    GameplayWorld, Interactable, ItemCatalog, ItemId, ItemInstanceId, ItemKind, ItemPickup,
    PlayerCommandFrame, PlayerController, PlayerInventory, PlayerWeaponState,
    SHARED_UNARMED_WEAPON_ITEM_NAME,
};
#[cfg(test)]
use newengine_gameplay_fps_api::action as fps_action;
use newengine_gameplay_fps_api::{
    FpsActionFrame, FpsCharacterMenuPolicySnapshot, FpsDemoState, FpsPlayableCharacterPolicy,
};
use newengine_ui_api::{UiEventDispatchFrame, UiNodeEventTrigger, UiStatePatch};

use newengine_fps_character_runtime::{fps_noclip_enabled, toggle_fps_noclip};
use newengine_fps_combat_runtime::focused_item_pickup;

#[path = "inventory_hud/character_variants.rs"]
mod character_variants;
#[path = "inventory_hud/commands.rs"]
mod commands;
#[path = "inventory_hud/helpers.rs"]
mod helpers;
#[path = "inventory_hud/interaction.rs"]
mod interaction;
#[path = "inventory_hud/provider.rs"]
mod provider;
#[path = "inventory_hud/publish.rs"]
mod publish;
#[path = "inventory_hud/state.rs"]
mod state;
#[cfg(test)]
#[path = "inventory_hud/tests.rs"]
mod tests;

pub use character_menu_policy::{
    ensure_character_menu_policy, ScriptFpsCharacterMenuPolicyProvider,
    SCRIPT_FPS_CHARACTER_MENU_PROVIDER_ID,
};
pub(crate) use character_variants::reconcile_existing_player_assignments_with_policy;
pub use commands::step_inventory_commands;
pub use provider::FpsInventoryHudProvider;
pub use state::character_select_is_open;

use character_variants::{
    availability_label, playable_character_variants, selected_variant, variant_from_action,
    PlayableCharacterSelection,
};
use helpers::*;
use interaction::apply_inventory_ui_actions;
#[cfg(test)]
use interaction::select_playable_character;
#[cfg(test)]
use interaction::{
    activate_inventory_instance, drop_instance_quantity, equip_dragged_instance, reorder_inventory,
};
use publish::publish_inventory_hud_state;
#[cfg(test)]
use state::inventory_hud_is_visible;
use state::{
    ensure_inventory_hud_state, inventory_hud_is_open, inventory_slot_count, CharacterMenuCategory,
    InventoryDragState, InventoryHudState, CHARACTER_UI_ACTION_CATEGORY_CHARACTERS,
    CHARACTER_UI_ACTION_CATEGORY_WEAPONS, CHARACTER_UI_ACTION_NOCLIP_TOGGLE,
    CHARACTER_UI_ACTION_TOGGLE, INVENTORY_HUD_CONTRACT, INVENTORY_HUD_SURFACE_ID,
    INVENTORY_UI_ACTION_DROP, INVENTORY_UI_ACTION_EQUIPMENT, INVENTORY_UI_ACTION_HOTBAR,
    INVENTORY_UI_ACTION_SLOT, INVENTORY_UI_ACTION_TOGGLE,
};
