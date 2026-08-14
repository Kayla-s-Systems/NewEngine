use super::*;

pub(super) fn toggle_inventory(world: &mut World) {
    ensure_inventory_hud_state(world);
    if let Some(state) = world.resource_mut::<InventoryHudState>() {
        state.toggle_inventory();
    }
}

pub(super) fn touch_hud_state(world: &mut World) {
    ensure_inventory_hud_state(world);
    if let Some(state) = world.resource_mut::<InventoryHudState>() {
        state.touch();
    }
}

pub(super) fn inventory_instance_at(
    world: &World,
    player: EntityId,
    index: usize,
) -> Option<ItemInstanceId> {
    world
        .get::<PlayerInventory>(player)
        .and_then(|inventory| inventory.entries.get(index))
        .map(|entry| entry.instance_id)
}

pub(super) fn parse_inventory_slot_index(world: &World, node_id: &str) -> Option<usize> {
    node_id
        .strip_prefix("inventory.slot.")?
        .parse::<usize>()
        .ok()
        .filter(|index| *index < inventory_slot_count(world))
}

pub(super) fn parse_hotbar_index(node_id: &str) -> Option<u8> {
    node_id
        .strip_prefix("inventory.hotbar.")?
        .parse::<u8>()
        .ok()
        .filter(|index| (1..=5).contains(index))
}

pub(super) fn parse_equipment_node(node_id: &str) -> Option<EquipmentSlot> {
    match node_id.strip_prefix("inventory.equipment.")? {
        "primary" => Some(EquipmentSlot::Primary),
        "secondary" => Some(EquipmentSlot::Secondary),
        "sidearm" => Some(EquipmentSlot::Sidearm),
        "melee" => Some(EquipmentSlot::Melee),
        "throwable" => Some(EquipmentSlot::Throwable),
        "gadget" => Some(EquipmentSlot::Gadget),
        "utility1" => Some(EquipmentSlot::Utility1),
        "utility2" => Some(EquipmentSlot::Utility2),
        _ => None,
    }
}

pub(super) fn hotbar_slot(index: u8) -> Option<EquipmentSlot> {
    match index {
        1 => Some(EquipmentSlot::Primary),
        2 => Some(EquipmentSlot::Secondary),
        3 => Some(EquipmentSlot::Sidearm),
        4 => Some(EquipmentSlot::Melee),
        5 => Some(EquipmentSlot::Throwable),
        _ => None,
    }
}

pub(super) const EQUIPMENT_SLOTS: [EquipmentSlot; 8] = [
    EquipmentSlot::Primary,
    EquipmentSlot::Secondary,
    EquipmentSlot::Sidearm,
    EquipmentSlot::Melee,
    EquipmentSlot::Throwable,
    EquipmentSlot::Gadget,
    EquipmentSlot::Utility1,
    EquipmentSlot::Utility2,
];

pub(super) fn equipment_slot_name(slot: EquipmentSlot) -> &'static str {
    match slot {
        EquipmentSlot::Primary => "primary",
        EquipmentSlot::Secondary => "secondary",
        EquipmentSlot::Sidearm => "sidearm",
        EquipmentSlot::Melee => "melee",
        EquipmentSlot::Throwable => "throwable",
        EquipmentSlot::Gadget => "gadget",
        EquipmentSlot::Utility1 => "utility1",
        EquipmentSlot::Utility2 => "utility2",
    }
}

pub(super) fn equipment_slot_code(slot: EquipmentSlot) -> u64 {
    match slot {
        EquipmentSlot::Primary => 1,
        EquipmentSlot::Secondary => 2,
        EquipmentSlot::Sidearm => 3,
        EquipmentSlot::Melee => 4,
        EquipmentSlot::Throwable => 5,
        EquipmentSlot::Gadget => 6,
        EquipmentSlot::Utility1 => 7,
        EquipmentSlot::Utility2 => 8,
    }
}
