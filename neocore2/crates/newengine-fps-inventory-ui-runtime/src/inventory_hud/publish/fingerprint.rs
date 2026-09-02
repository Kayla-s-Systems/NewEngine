pub(super) fn inventory_hud_fingerprint(world: &World, player: EntityId) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut push = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    if let Some(state) = world.resource::<InventoryHudState>() {
        push(state.visible as u64);
        push(state.open as u64);
        push(state.character_select_open as u64);
        push(state.character_category.fingerprint_code());
        push(state.character_nav_index as u64);
        push(state.revision);
        push(state.selected_instance.map_or(0, |instance| instance.0));
        push(state.drag.map_or(0, |drag| drag.instance_id.0));
    }
    let character_menu_open = world
        .resource::<InventoryHudState>()
        .is_some_and(|state| state.character_select_open);
    push(fps_noclip_enabled(world, player) as u64);
    if let Some(inventory) = world.get::<PlayerInventory>(player) {
        push(inventory.entries.len() as u64);
        push(inventory.active_slot.map_or(0, equipment_slot_code));
        for entry in &inventory.entries {
            push(entry.instance_id.0);
            push(entry.item.0);
            push(u64::from(entry.quantity));
            push(u64::from(entry.condition.to_bits()));
        }
        for (slot, instance) in &inventory.equipped {
            push(equipment_slot_code(*slot));
            push(instance.0);
        }
    }
    push(focused_item_pickup(world, player).map_or(0, EntityId::stable_u64));
    if character_menu_open {
        if let Some(binding) =
            world.get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
        {
            push(binding.assignment_revision);
            for byte in binding.source.as_bytes() {
                push(u64::from(*byte));
            }
        }
        if let Some(selection) = world.get::<PlayableCharacterSelection>(player) {
            for byte in selection.variant_id.as_bytes() {
                push(u64::from(*byte));
            }
        }
        for variant in playable_character_variants(world) {
            for byte in variant
                .id
                .as_bytes()
                .iter()
                .chain(variant.display_name.as_bytes())
                .chain(variant.family.as_bytes())
            {
                push(u64::from(*byte));
            }
            push(variant.runtime_ready as u64);
            if let Some(reference) = variant.runtime_model_ref.as_deref() {
                for byte in reference.as_bytes() {
                    push(u64::from(*byte));
                }
            }
        }
        if let Some(menu) = world.resource::<FpsCharacterMenuPolicySnapshot>() {
            for byte in menu
                .toggle_action
                .as_bytes()
                .iter()
                .chain(menu.title.as_bytes())
            {
                push(u64::from(*byte));
            }
            push(menu.characters.len() as u64);
            for character in &menu.characters {
                for byte in character.id.as_bytes() {
                    push(u64::from(*byte));
                }
            }
        }
    }
    if let Some(vitals) = character_vitals_hud_model(world, player) {
        push(vitals.entity);
        push(vitals.alive as u64);
        push(vitals.control_enabled as u64);
        push(u64::from(vitals.health_current.to_bits()));
        push(u64::from(vitals.health_maximum.to_bits()));
        push(u64::from(vitals.stamina_current.to_bits()));
        push(u64::from(vitals.stamina_maximum.to_bits()));
        push(vitals.stamina_exhausted as u64);
        push(vitals.injured as u64);
        push(match vitals.hit_reaction {
            newengine_engine_runtime::gameplay::CharacterHitReactionKind::None => 0,
            newengine_engine_runtime::gameplay::CharacterHitReactionKind::Flinch => 1,
            newengine_engine_runtime::gameplay::CharacterHitReactionKind::Stagger => 2,
        });
        push(vitals.damage_flash as u64);
        push(match vitals.death_phase {
            None => 0,
            Some(newengine_engine_runtime::gameplay::CharacterDeathPhase::TransitionRequested) => 1,
            Some(newengine_engine_runtime::gameplay::CharacterDeathPhase::Corpse) => 2,
        });
    }
    if let Some(weapon) = world.get::<PlayerWeaponState>(player) {
        push(u64::from(weapon.ammo_in_magazine));
        push(u64::from(weapon.reserve_ammo));
    }
    if let Some(mission) = world.resource::<FpsObjectiveState>() {
        push(u64::from(mission.pickups_collected));
        push(u64::from(mission.pickups_total));
        push(u64::from(mission.targets_destroyed));
        push(u64::from(mission.targets_total));
        push(mission.completed as u64);
        push(mission.failed as u64);
        for byte in mission.status.as_bytes() {
            push(u64::from(*byte));
        }
    }
    hash
}
