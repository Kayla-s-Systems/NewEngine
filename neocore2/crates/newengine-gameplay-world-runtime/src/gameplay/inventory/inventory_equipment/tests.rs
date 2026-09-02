use super::*;

fn component(
    id: &str,
    slot: &str,
    accuracy: f32,
    recoil: f32,
    damage: f32,
) -> WeaponComponentDefinition {
    WeaponComponentDefinition {
        id: id.to_owned(),
        slot: slot.to_owned(),
        model_ref: None,
        audio_override: None,
        muzzle_vfx_override: None,
        tracer_vfx_override: None,
        stat_modifiers: WeaponStatModifierStack::default(),
        modifiers: WeaponComponentModifiers {
            accuracy_multiplier: accuracy,
            recoil_multiplier: recoil,
            damage_multiplier: damage,
            ..WeaponComponentModifiers::default()
        },
    }
}

#[test]
fn component_install_validates_slot_and_aggregates_active_instance_modifiers() {
    let mut world = World::new();
    let owner = world.spawn();
    let ammo = ItemId::from_name("ammo.component.test").expect("ammo id");
    let weapon_id = ItemId::from_name("weapon.component.test").expect("weapon id");

    let graph = WeaponComponentGraphDefinition {
        points: vec![
            WeaponComponentPointDefinition {
                id: "muzzle".to_owned(),
                attach_joint: "muzzle".to_owned(),
                allowed_components: vec!["muzzle.standard".to_owned(), "muzzle.brake".to_owned()],
            },
            WeaponComponentPointDefinition {
                id: "optic".to_owned(),
                attach_joint: "optic".to_owned(),
                allowed_components: vec!["optic.basic".to_owned()],
            },
        ],
        components: [
            (
                "muzzle.standard".to_owned(),
                component("muzzle.standard", "muzzle", 1.0, 1.0, 1.0),
            ),
            (
                "muzzle.brake".to_owned(),
                component("muzzle.brake", "muzzle", 0.9, 0.7, 1.05),
            ),
            (
                "optic.basic".to_owned(),
                component("optic.basic", "optic", 0.8, 1.0, 1.0),
            ),
        ]
        .into_iter()
        .collect(),
        default_installed: [("muzzle".to_owned(), "muzzle.standard".to_owned())]
            .into_iter()
            .collect(),
    };

    let weapon = ItemDefinition::weapon(
        "weapon.component.test",
        "Component Test Weapon",
        EquipmentSlot::Primary,
        HitscanWeaponTuning::default(),
        ammo,
        WeaponFireMode::SemiAuto,
        2.5,
    )
    .expect("weapon")
    .with_weapon_components(graph)
    .expect("component graph");
    let mut catalog = ItemCatalog::default();
    catalog.register(weapon).expect("register weapon");
    world.insert_resource(catalog);

    let mutation = give_item(&mut world, owner, weapon_id, 1).expect("give weapon");
    let instance = *mutation.touched_instances.first().expect("weapon instance");
    equip_item_instance(&mut world, owner, instance).expect("equip weapon");

    let defaults = active_equipped_weapon_component_modifiers(&world, owner);
    assert!((defaults.recoil_multiplier - 1.0).abs() < 1.0e-6);

    assert!(
        install_weapon_component(&mut world, owner, instance, "muzzle", "optic.basic").is_err(),
        "component from another slot must be rejected"
    );
    install_weapon_component(&mut world, owner, instance, "muzzle", "muzzle.brake")
        .expect("install muzzle brake");

    let modified = active_equipped_weapon_component_modifiers(&world, owner);
    assert!((modified.accuracy_multiplier - 0.9).abs() < 1.0e-6);
    assert!((modified.recoil_multiplier - 0.7).abs() < 1.0e-6);
    assert!((modified.damage_multiplier - 1.05).abs() < 1.0e-6);

    let removed =
        remove_weapon_component(&mut world, owner, instance, "muzzle").expect("remove component");
    assert_eq!(removed.component_id, "muzzle.brake");
    assert_eq!(
        active_equipped_weapon_component_modifiers(&world, owner),
        WeaponComponentModifiers::default()
    );
}

#[test]
fn ordinary_healing_item_cannot_revive_dead_character() {
    let mut world = World::new();
    let owner = world.spawn();
    let medkit = ItemDefinition::consumable(
        "consumable.dead-heal.test",
        "Dead Heal Test",
        2,
        0.1,
        ItemUseEffect::Heal { amount: 50.0 },
    )
    .expect("medkit definition");
    let medkit_id = medkit.id;
    let mut catalog = ItemCatalog::default();
    catalog.register(medkit).expect("register medkit");
    world.insert_resource(catalog);
    let _ = world.insert(owner, Health::new(100.0));
    world.get_mut::<Health>(owner).unwrap().current = 0.0;
    let _ = world.insert(owner, CharacterLifeState::Dead);
    give_item(&mut world, owner, medkit_id, 1).expect("give medkit");

    let result = use_item(&mut world, owner, medkit_id);

    assert!(result.is_err());
    assert_eq!(world.get::<Health>(owner).unwrap().current, 0.0);
    assert_eq!(inventory_quantity(&world, owner, medkit_id), 1);
    assert_eq!(
        world.get::<CharacterLifeState>(owner).copied(),
        Some(CharacterLifeState::Dead)
    );
}

#[test]
fn healing_crossing_injury_threshold_recovers_injury_immediately() {
    let mut world = World::new();
    let owner = world.spawn();
    let medkit = ItemDefinition::consumable(
        "consumable.injury-recovery.test",
        "Injury Recovery Test",
        2,
        0.1,
        ItemUseEffect::Heal { amount: 50.0 },
    )
    .expect("medkit definition");
    let medkit_id = medkit.id;
    let mut catalog = ItemCatalog::default();
    catalog.register(medkit).expect("register medkit");
    world.insert_resource(catalog);
    let _ = world.insert(
        owner,
        Health {
            current: 20.0,
            maximum: 100.0,
        },
    );
    let _ = world.insert(owner, CharacterLifeState::Alive);
    let _ = world.insert(
        owner,
        crate::gameplay::CharacterInjuryState {
            injured: true,
            revision: 1,
        },
    );
    give_item(&mut world, owner, medkit_id, 1).expect("give medkit");

    use_item(&mut world, owner, medkit_id).expect("use healing item");

    assert_eq!(world.get::<Health>(owner).unwrap().current, 70.0);
    assert!(world
        .get::<crate::gameplay::CharacterInjuryState>(owner)
        .is_some_and(|state| !state.injured));
    let events = crate::gameplay::drain_gameplay_events(&mut world);
    assert_eq!(
        events
            .iter()
            .map(|event| event.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            crate::gameplay::GAMEPLAY_EVENT_CHARACTER_HEALED,
            crate::gameplay::GAMEPLAY_EVENT_CHARACTER_INJURY_RECOVERED,
        ]
    );
}
