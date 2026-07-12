use super::*;

pub const INVENTORY_HUD_SURFACE_ID: &str = "game.hud";
pub const INVENTORY_HUD_CONTRACT: &str = "newengine.gameplay.inventory_hud.snapshot.v1";
pub const INVENTORY_UI_ACTION_TOGGLE: &str = "game.inventory.toggle";
pub const INVENTORY_UI_ACTION_SLOT: &str = "game.inventory.slot";
pub const INVENTORY_UI_ACTION_HOTBAR: &str = "game.inventory.hotbar";
pub const INVENTORY_UI_ACTION_EQUIPMENT: &str = "game.inventory.equipment";
pub const INVENTORY_UI_ACTION_DROP: &str = "game.inventory.drop";
pub(super) const INVENTORY_SLOT_COUNT: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryDragState {
    pub instance_id: ItemInstanceId,
    pub source_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryHudState {
    pub visible: bool,
    pub open: bool,
    pub selected_instance: Option<ItemInstanceId>,
    pub drag: Option<InventoryDragState>,
    pub revision: u64,
    pub last_published_hash: u64,
    pub last_published_frame: u64,
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
            self.drag = None;
            self.selected_instance = None;
        }
        self.touch();
    }

    pub(super) fn toggle_inventory(&mut self) {
        self.visible = true;
        self.open = !self.open;
        self.drag = None;
        if !self.open {
            self.selected_instance = None;
        }
        self.touch();
    }
}

impl Default for InventoryHudState {
    fn default() -> Self {
        Self {
            visible: true,
            open: false,
            selected_instance: None,
            drag: None,
            revision: 0,
            last_published_hash: 0,
            last_published_frame: 0,
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

#[inline]
pub fn inventory_hud_is_visible(world: &World) -> bool {
    world
        .resource::<InventoryHudState>()
        .map(|state| state.visible)
        .unwrap_or(true)
}
