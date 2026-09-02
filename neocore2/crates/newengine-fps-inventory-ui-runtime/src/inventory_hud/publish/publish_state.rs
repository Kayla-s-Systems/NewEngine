use super::*;

pub(super) fn publish_inventory_hud_state(
    world: &mut World,
    frame_index: u64,
) -> GameplayUiFrameOutput {
    ensure_inventory_hud_state(world);
    let Some(player) = first_player(world) else {
        return GameplayUiFrameOutput::default();
    };
    let fingerprint = inventory_hud_fingerprint(world, player);
    if world
        .resource::<InventoryHudState>()
        .is_some_and(|state| state.last_published_hash == fingerprint)
    {
        return GameplayUiFrameOutput::default();
    }

    let (patch, visible) = {
        let state = world
            .resource::<InventoryHudState>()
            .expect("inventory HUD state initialized");
        let inventory = world.get::<PlayerInventory>(player);
        let catalog = world.resource::<ItemCatalog>();
        let mission = world.resource::<FpsObjectiveState>();
        let weapon_state = world.get::<PlayerWeaponState>(player).copied();
        let binding = world.get::<EquippedWeaponBinding>(player).copied();
        let vitals = character_vitals_hud_model(world, player);

        let empty_inventory = PlayerInventory::default();
        let empty_catalog = ItemCatalog::default();
        let inventory = inventory.unwrap_or(&empty_inventory);
        let catalog = catalog.unwrap_or(&empty_catalog);
        let total_weight = inventory.total_weight(catalog);

        let mut patch = base_patch(
            frame_index,
            state,
            inventory,
            mission,
            weapon_state,
            vitals,
            total_weight,
            catalog,
        );
        let focused_pickup = focused_item_pickup(world, player);
        let focused_item_name = focused_pickup
            .and_then(|entity| world.get::<ItemPickup>(entity))
            .and_then(|pickup| catalog.get(pickup.item))
            .map(|definition| definition.display_name.as_str())
            .unwrap_or("");
        let focused_prompt = focused_pickup
            .and_then(|entity| world.get::<Interactable>(entity))
            .map(|interactable| interactable.prompt.as_str())
            .unwrap_or("");
        patch = patch
            .with_change(
                "interaction",
                "visible",
                serde_json::json!(focused_pickup.is_some()),
            )
            .with_change(
                "interaction",
                "action_label",
                serde_json::json!("Взять предмет"),
            )
            .with_change("interaction", "key_label", serde_json::json!("E"))
            .with_change(
                "interaction",
                "item_name",
                serde_json::json!(focused_item_name),
            )
            .with_change("interaction", "prompt", serde_json::json!(focused_prompt));
        let selected = selected_variant(world, player);
        let fallback_source = world
            .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
            .map(|binding| binding.source.as_str())
            .unwrap_or("");
        let character_title = world
            .resource::<FpsCharacterMenuPolicySnapshot>()
            .map(|policy| policy.title.as_str())
            .unwrap_or("Character");
        let noclip_enabled = fps_noclip_enabled(world, player);
        patch = patch
            .with_change(
                "character",
                "open",
                serde_json::json!(state.character_select_open),
            )
            .with_change("character", "title", serde_json::json!(character_title))
            .with_change(
                "character",
                "noclip_enabled",
                serde_json::json!(noclip_enabled),
            )
            .with_change(
                "character",
                "noclip_label",
                serde_json::json!(if noclip_enabled {
                    "NoClip - ENABLED"
                } else {
                    "NoClip - Disabled"
                }),
            )
            .with_change(
                "character",
                "selected",
                serde_json::json!(selected
                    .map(|variant| variant.display_name.as_str())
                    .unwrap_or(fallback_source)),
            )
            .with_change(
                "character",
                "selected_id",
                serde_json::json!(selected
                    .map(|variant| variant.id.as_str())
                    .unwrap_or("unknown")),
            )
            .with_change(
                "character",
                "selected_family",
                serde_json::json!(selected
                    .map(|variant| variant.family.as_str())
                    .unwrap_or("Unknown")),
            )
            .with_change(
                "character",
                "selected_rig",
                serde_json::json!(selected
                    .map(|variant| variant.rig_label.as_str())
                    .unwrap_or("Unspecified rig")),
            )
            .with_change(
                "character",
                "selected_description",
                serde_json::json!(selected
                    .map(|variant| variant.subtitle.as_str())
                    .unwrap_or("External player model assignment")),
            )
            .with_change(
                "character",
                "selected_status",
                serde_json::json!(selected
                    .map(availability_label)
                    .unwrap_or("External assignment")),
            );
        let character_category = state.character_category;
        patch = patch
            .with_change(
                "character",
                "category_characters_selected",
                serde_json::json!(matches!(
                    character_category,
                    CharacterMenuCategory::Characters
                )),
            )
            .with_change(
                "character",
                "category_weapons_selected",
                serde_json::json!(matches!(character_category, CharacterMenuCategory::Weapons)),
            )
            .with_change(
                "character",
                "characters_visible",
                serde_json::json!(matches!(
                    character_category,
                    CharacterMenuCategory::Characters
                )),
            )
            .with_change(
                "character",
                "weapons_visible",
                serde_json::json!(matches!(character_category, CharacterMenuCategory::Weapons)),
            );
        for (index, variant) in playable_character_variants(world).iter().enumerate() {
            patch = patch.with_change(
                "character",
                format!("nav_{}", variant.id),
                serde_json::json!(
                    state.character_select_open && state.character_nav_index == index
                ),
            );
        }
        if state.character_select_open {
            let registered_characters = registered_character_items(world, player);
            patch = patch
                .with_change(
                    "character",
                    "registered_character_count",
                    serde_json::json!(registered_characters.len()),
                )
                .with_change(
                    "character",
                    "registered_character_count_label",
                    serde_json::json!(format!("{} AVAILABLE", registered_characters.len())),
                )
                .with_change(
                    "character",
                    "registered_characters",
                    serde_json::Value::Array(registered_characters),
                );

            let registered_weapons = registered_weapon_items(catalog, binding);
            let registered_weapon_count = registered_weapons.len();
            patch = patch
                .with_change(
                    "character",
                    "registered_weapons",
                    serde_json::Value::Array(registered_weapons),
                )
                .with_change(
                    "character",
                    "registered_weapon_count",
                    serde_json::json!(registered_weapon_count),
                )
                .with_change(
                    "character",
                    "registered_weapon_count_label",
                    serde_json::json!(format!("{} AVAILABLE", registered_weapon_count)),
                );
        }
        let selected_weapon = binding.and_then(|binding| catalog.get(binding.item));
        patch = patch
            .with_change(
                "character",
                "selected_weapon_name",
                serde_json::json!(selected_weapon
                    .map(|definition| definition.display_name.as_str())
                    .unwrap_or("Unarmed")),
            )
            .with_change(
                "character",
                "selected_weapon_description",
                serde_json::json!(selected_weapon
                    .map(|definition| definition.description.as_str())
                    .filter(|description| !description.trim().is_empty())
                    .unwrap_or("No weapon is currently equipped.")),
            )
            .with_change(
                "character",
                "selected_weapon_status",
                serde_json::json!(if selected_weapon.is_some() {
                    "Equipped · runtime active"
                } else {
                    "Unarmed · no active weapon"
                }),
            )
            .with_change(
                "character",
                "selected_weapon_slot",
                serde_json::json!(selected_weapon
                    .and_then(|definition| definition.equipment_slot)
                    .map(equipment_slot_name)
                    .unwrap_or("none")),
            )
            .with_change(
                "character",
                "selected_weapon_id",
                serde_json::json!(selected_weapon
                    .map(|definition| definition.name.as_str())
                    .unwrap_or("unarmed")),
            );

        patch = patch.with_change(
            "character",
            "nav_noclip",
            serde_json::json!(
                state.character_select_open
                    && state.character_nav_index == playable_character_variants(world).len()
            ),
        );
        for index in 0..inventory_slot_count(world) {
            patch = patch_inventory_slot(patch, index, state, inventory, catalog);
        }
        for index in 1..=5u8 {
            patch = patch_hotbar_slot(patch, index, inventory, catalog);
        }
        for slot in EQUIPMENT_SLOTS {
            patch = patch_equipment_slot(patch, slot, state.open, inventory, catalog);
        }
        if let Some(binding) = binding {
            patch = patch.with_change(
                "inventory",
                "active_weapon_instance",
                serde_json::json!(binding.instance_id.0.to_string()),
            );
        }
        (patch, state.visible)
    };

    let visibility_changed = world
        .resource::<InventoryHudState>()
        .is_none_or(|state| state.last_published_visible != Some(visible));
    if let Some(state) = world.resource_mut::<InventoryHudState>() {
        state.last_published_hash = fingerprint;
        state.last_published_frame = frame_index;
        if visibility_changed {
            state.last_published_visible = Some(visible);
        }
    }

    let mut output = GameplayUiFrameOutput::default().with_patch(
        patch,
        "gameplay.fps.inventory",
        INVENTORY_HUD_CONTRACT,
    );
    if visibility_changed {
        output = output.with_surface_visibility(INVENTORY_HUD_SURFACE_ID, visible);
    }
    output
}
