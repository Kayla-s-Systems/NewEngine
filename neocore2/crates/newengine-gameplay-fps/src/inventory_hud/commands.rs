use super::*;

pub fn step_inventory_commands(world: &mut World, _fixed_tick: u64) {
    ensure_inventory_hud_state(world);
    let players = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .collect::<Vec<_>>();
    for player in players {
        let actions = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| FpsActionFrame::from_commands(&commands.actions))
            .unwrap_or_default();
        if actions.hud_visibility_toggle_pressed {
            if let Some(state) = world.resource_mut::<InventoryHudState>() {
                state.toggle_visibility();
                newengine_ulog_api::ulog::info!(
                    "game HUD visibility toggled visible={} source='game.hud.visibility.toggle'",
                    state.visible
                );
            }
        }
        if actions.inventory_toggle_pressed {
            if let Some(state) = world.resource_mut::<InventoryHudState>() {
                state.toggle_inventory();
            }
        }
        if let Some(index) = actions.equipment_slot_pressed {
            if let Some(slot) = hotbar_slot(index) {
                if select_equipment_slot(world, player, slot).is_ok() {
                    touch_hud_state(world);
                }
            }
        }
    }
}
