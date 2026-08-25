use super::*;

pub fn step_inventory_commands(world: &mut World, _fixed_tick: u64) {
    ensure_inventory_hud_state(world);
    let players = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .collect::<Vec<_>>();
    for player in players {
        let (source_frame, actions) = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| {
                (
                    commands.source_frame,
                    FpsActionFrame::from_commands(&commands.actions),
                )
            })
            .unwrap_or_default();
        const CHARACTER_TOGGLE_RELEASE_REARM_SAMPLES: u16 = 90;
        let character_toggle_edge = {
            let state = world
                .resource_mut::<InventoryHudState>()
                .expect("inventory HUD state initialized");
            if actions.character_select_toggle_pressed {
                state.character_toggle_release_streak = 0;
                if state.character_toggle_latched {
                    false
                } else {
                    state.character_toggle_latched = true;
                    true
                }
            } else if state.character_toggle_latched {
                state.character_toggle_release_streak =
                    state.character_toggle_release_streak.saturating_add(1);
                if state.character_toggle_release_streak >= CHARACTER_TOGGLE_RELEASE_REARM_SAMPLES {
                    state.character_toggle_latched = false;
                    state.character_toggle_release_streak = 0;
                }
                false
            } else {
                state.character_toggle_release_streak = 0;
                false
            }
        };
        let pulse_already_consumed = world
            .resource::<InventoryHudState>()
            .and_then(|state| state.last_consumed_pulse_source_frame)
            == Some(source_frame);
        if pulse_already_consumed {
            continue;
        }
        if let Some(state) = world.resource_mut::<InventoryHudState>() {
            state.last_consumed_pulse_source_frame = Some(source_frame);
        }
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
        if character_toggle_edge {
            let variants = playable_character_variants(world);
            let variant_count = variants.len();
            let selected_index = selected_variant(world, player)
                .and_then(|selected| {
                    variants
                        .iter()
                        .position(|variant| variant.id == selected.id)
                })
                .unwrap_or(0);
            if let Some(state) = world.resource_mut::<InventoryHudState>() {
                if variant_count == 0 {
                    state.close_character_select();
                } else {
                    state.toggle_character_select();
                }
                if state.character_select_open {
                    state.set_character_nav_index(selected_index, variant_count);
                }
                newengine_ulog_api::ulog::info!(
                    "character selector toggled open={} source='player.character.select.toggle'",
                    state.character_select_open
                );
            }
        }

        if character_select_is_open(world) {
            let previous = actions.ui_nav_up_pressed || actions.ui_nav_left_pressed;
            let next = actions.ui_nav_down_pressed || actions.ui_nav_right_pressed;
            let variant_count = playable_character_variants(world).len();
            if previous {
                if let Some(state) = world.resource_mut::<InventoryHudState>() {
                    state.navigate_character_select(-1, variant_count);
                }
            }
            if next {
                if let Some(state) = world.resource_mut::<InventoryHudState>() {
                    state.navigate_character_select(1, variant_count);
                }
            }
            if actions.ui_back_pressed {
                if let Some(state) = world.resource_mut::<InventoryHudState>() {
                    state.close_character_select();
                }
            } else if actions.ui_accept_pressed {
                let index = world
                    .resource::<InventoryHudState>()
                    .map(|state| state.character_nav_index)
                    .unwrap_or(0)
                    .min(playable_character_variants(world).len().saturating_sub(1));
                let variant = playable_character_variants(world).get(index).cloned();
                if let Some(variant) = variant.as_ref() {
                    let _ = select_playable_character(world, player, variant);
                }
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
