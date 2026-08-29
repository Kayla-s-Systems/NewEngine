use super::*;

pub const INVENTORY_HUD_SURFACE_ID: &str = "game.hud";
pub const INVENTORY_HUD_CONTRACT: &str = "newengine.gameplay.inventory_hud.snapshot.v1";
pub const INVENTORY_UI_ACTION_TOGGLE: &str = "game.inventory.toggle";
pub const INVENTORY_UI_ACTION_SLOT: &str = "game.inventory.slot";
pub const INVENTORY_UI_ACTION_HOTBAR: &str = "game.inventory.hotbar";
pub const INVENTORY_UI_ACTION_EQUIPMENT: &str = "game.inventory.equipment";
pub const INVENTORY_UI_ACTION_DROP: &str = "game.inventory.drop";
pub const CHARACTER_UI_ACTION_TOGGLE: &str = "game.character.toggle";
pub const CHARACTER_UI_ACTION_NOCLIP_TOGGLE: &str = "game.character.noclip.toggle";

#[inline]
pub(super) fn inventory_slot_count(world: &World) -> usize {
    crate::game_data::active_game_data(world)
        .map(|data| data.gameplay.inventory.hud_slots.clamp(1, 256))
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryDragState {
    pub instance_id: ItemInstanceId,
    pub source_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryHudState {
    pub visible: bool,
    pub open: bool,
    pub character_select_open: bool,
    /// Stable keyboard/gamepad focus for the vertical playable-character list.
    pub character_nav_index: usize,
    /// Debounces a logical M press across input snapshots. Some providers keep the
    /// pressed pulse asserted for several sampled frames while the key transitions.
    pub character_toggle_latched: bool,
    pub selected_instance: Option<ItemInstanceId>,
    pub drag: Option<InventoryDragState>,
    pub revision: u64,
    pub last_published_hash: u64,
    pub last_published_frame: u64,
    /// Surface visibility is edge-published only; repeated true/true updates used to invalidate
    /// the egui frame cache and made the menu visibly blink during unrelated HUD updates.
    pub last_published_visible: Option<bool>,
    /// Edge-triggered UI/gameplay actions are sampled once per render/input frame
    /// but this system can run multiple fixed ticks for that same sample.
    /// Remember the sample frame so a single M press cannot toggle the selector
    /// open/closed repeatedly during fixed-step catch-up.
    pub last_consumed_pulse_source_frame: Option<u64>,
}

impl InventoryHudState {
    #[inline]
    pub(super) fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(super) fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.open = false;
            self.character_select_open = false;
            self.drag = None;
            self.selected_instance = None;
        }
        self.touch();
    }

    pub(super) fn toggle_inventory(&mut self) {
        self.visible = true;
        self.open = !self.open;
        self.character_select_open = false;
        self.drag = None;
        if !self.open {
            self.selected_instance = None;
        }
        self.touch();
    }

    pub(super) fn toggle_character_select(&mut self) {
        self.visible = true;
        self.character_select_open = !self.character_select_open;
        self.open = false;
        self.drag = None;
        self.selected_instance = None;
        self.touch();
    }

    pub(super) fn set_character_nav_index(&mut self, index: usize, count: usize) {
        if count == 0 {
            return;
        }
        let index = index.min(count - 1);
        if self.character_nav_index != index {
            self.character_nav_index = index;
            self.touch();
        }
    }

    pub(super) fn navigate_character_select(&mut self, delta: isize, count: usize) {
        if !self.character_select_open || count == 0 || delta == 0 {
            return;
        }
        let current = self.character_nav_index.min(count - 1) as isize;
        let next = (current + delta).rem_euclid(count as isize) as usize;
        self.set_character_nav_index(next, count);
    }

    pub(super) fn close_character_select(&mut self) {
        if self.character_select_open {
            self.character_select_open = false;
            self.touch();
        }
    }
}

impl Default for InventoryHudState {
    fn default() -> Self {
        Self {
            visible: true,
            open: false,
            character_select_open: false,
            character_nav_index: 0,
            character_toggle_latched: false,
            selected_instance: None,
            drag: None,
            revision: 0,
            last_published_hash: 0,
            last_published_frame: 0,
            last_published_visible: None,
            last_consumed_pulse_source_frame: None,
        }
    }
}

#[inline]
pub fn ensure_inventory_hud_state(world: &mut World) {
    if world.resource::<InventoryHudState>().is_none() {
        world.insert_resource(InventoryHudState::default());
    }
}

#[inline]
pub fn inventory_hud_is_open(world: &World) -> bool {
    world
        .resource::<InventoryHudState>()
        .map(|state| state.open)
        .unwrap_or(false)
}

#[cfg(test)]
#[inline]
pub fn inventory_hud_is_visible(world: &World) -> bool {
    world
        .resource::<InventoryHudState>()
        .map(|state| state.visible)
        .unwrap_or(true)
}

#[inline]
pub fn character_select_is_open(world: &World) -> bool {
    world
        .resource::<InventoryHudState>()
        .map(|state| state.character_select_open)
        .unwrap_or(false)
}
