use super::*;

pub fn step_inventory_commands(world: &mut World, _fixed_tick: u64) {
    ensure_inventory_hud_state(world);
    let toggle_action = world
        .resource::<FpsCharacterMenuPolicySnapshot>()
        .map(|policy| policy.toggle_action.clone());
    let players = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .collect::<Vec<_>>();
    for player in players {
        let (source_frame, actions, character_toggle_pressed, character_toggle_released) = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| {
                (
                    commands.source_frame,
                    FpsActionFrame::from_commands(&commands.actions),
                    toggle_action
                        .as_deref()
                        .is_some_and(|action| commands.actions.is_pressed(action)),
                    toggle_action
                        .as_deref()
                        .is_some_and(|action| commands.actions.is_released(action)),
                )
            })
            .unwrap_or_default();
        let character_toggle_edge = {
            let state = world
                .resource_mut::<InventoryHudState>()
                .expect("inventory HUD state initialized");

            // Explicit release re-arms M even while the selector owns gameplay focus.
            // The next M press can therefore close the modal immediately, while one
            // sampled press still cannot toggle twice during fixed-step catch-up.
            // Release is authoritative for this render/input sample. If no fixed tick
            // has consumed the previous press yet, PlayerCommandFrame may legally carry
            // both the stale `pressed` pulse and the fresh `released` pulse. Re-arm the
            // latch, but never replay that stale press as a second toggle.
            if character_toggle_released {
                state.character_toggle_latched = false;
                false
            } else if character_toggle_pressed && !state.character_toggle_latched {
                state.character_toggle_latched = true;
                true
            } else {
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
        // The configured character-toggle semantic action is the only gameplay action owned by
        // this modal. Once open, retained engine.ui focus owns keyboard/gamepad navigation and
        // activation, avoiding competing focus indices in gameplay code and the UI provider.
        if character_toggle_edge {
            let variants = playable_character_variants(world);
            let variant_count = variants.len();
            let menu_entry_count = variant_count + 1; // playable characters + No Clip checkbox
            let selected_index = selected_variant(world, player)
                .and_then(|selected| {
                    variants
                        .iter()
                        .position(|variant| variant.id == selected.id)
                })
                .unwrap_or(0);
            if let Some(state) = world.resource_mut::<InventoryHudState>() {
                state.toggle_character_select();
                if state.character_select_open {
                    state.set_character_nav_index(selected_index, menu_entry_count);
                }
                newengine_ulog_api::ulog::info!(
                    "character selector toggled open={} source='{}' input_owner='engine.ui.retained-focus'",
                    state.character_select_open,
                    toggle_action.as_deref().unwrap_or("<unconfigured>")
                );
            }
            // A physical M edge is exclusive for this frame. Do not let a simultaneous gameplay
            // hotkey leak through while the modal is changing capture ownership.
            continue;
        }

        if character_select_is_open(world) {
            // All menu navigation/activation is dispatched by Aurelia from raw keyboard/gamepad
            // input. Keep gameplay hotkeys inert while the menu owns focus.
            continue;
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
                newengine_ulog_api::ulog::info!(
                    "inventory HUD toggled open={} source='player.inventory.toggle'",
                    state.open
                );
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
