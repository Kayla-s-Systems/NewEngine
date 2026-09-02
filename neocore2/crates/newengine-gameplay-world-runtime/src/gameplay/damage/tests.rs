use super::*;

#[test]
fn character_hit_zone_and_armor_are_resolved_outside_weapon_runtime() {
    let mut world = World::new();
    let source = world.spawn();
    let target = world.spawn();
    let _ = world.insert(target, Health::new(100.0));
    let _ = world.insert(
        target,
        DamageReceiver {
            kind: DamageReceiverKind::Character,
            damage_multiplier: 1.0,
            armor_absorption: 0.20,
            impulse_multiplier: 1.0,
        },
    );
    let _ = world.insert(
        target,
        DamageHitZoneMap {
            by_subshape: [(
                7,
                DamageHitZone {
                    id: "head".to_owned(),
                    damage_multiplier: 2.0,
                    armor_absorption: 0.0,
                    impulse_multiplier: 1.25,
                },
            )]
            .into_iter()
            .collect(),
        },
    );
    let resolution = resolve_weapon_impact(
        &mut world,
        WeaponImpact {
            sequence: 1,
            source,
            target,
            base_damage: 25.0,
            point: Vec3::ZERO,
            normal: Vec3::Y,
            direction: -Vec3::Z,
            distance: 0.0,
            range: 100.0,
            subshape_id: 7,
            momentum_ns: 3.0,
            ammo_impulse_multiplier: 1.0,
            falloff_multiplier: 1.0,
        },
    )
    .expect("authored receiver");
    assert_eq!(resolution.receiver_kind, DamageReceiverKind::Character);
    assert_eq!(resolution.hit_zone.as_deref(), Some("head"));
    assert!((resolution.applied_damage - 40.0).abs() < 1.0e-4);
    assert!(world.get::<PendingPhysicsImpulse>(target).is_some());
}
#[test]
fn lethal_character_damage_transitions_once_and_publishes_semantic_events() {
    let mut world = World::new();
    let source = world.spawn();
    let target = world.spawn();
    let _ = world.insert(target, Health::new(20.0));
    let _ = world.insert(target, crate::gameplay::CharacterLifeState::Alive);
    let _ = world.insert(target, DamageReceiver::character());

    let impact = WeaponImpact {
        sequence: 9,
        source,
        target,
        base_damage: 25.0,
        point: Vec3::ZERO,
        normal: Vec3::Y,
        direction: -Vec3::Z,
        distance: 1.0,
        range: 100.0,
        subshape_id: 0,
        momentum_ns: 2.0,
        ammo_impulse_multiplier: 1.0,
        falloff_multiplier: 1.0,
    };
    let resolution = resolve_weapon_impact(&mut world, impact).expect("character impact");
    assert_eq!(resolution.applied_damage, 20.0);
    assert_eq!(
        world
            .get::<crate::gameplay::CharacterLifeState>(target)
            .copied(),
        Some(crate::gameplay::CharacterLifeState::Dead)
    );
    let events = crate::gameplay::drain_gameplay_events(&mut world);
    assert_eq!(
        events
            .iter()
            .map(|event| event.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            crate::gameplay::GAMEPLAY_EVENT_CHARACTER_DAMAGED,
            crate::gameplay::GAMEPLAY_EVENT_CHARACTER_DIED,
            crate::gameplay::GAMEPLAY_EVENT_CHARACTER_DEATH_PRESENTATION_REQUESTED,
        ]
    );

    let second = resolve_weapon_impact(
        &mut world,
        WeaponImpact {
            sequence: 10,
            ..impact
        },
    )
    .expect("second impact");
    assert_eq!(second.applied_damage, 0.0);
    assert!(crate::gameplay::drain_gameplay_events(&mut world).is_empty());
}

#[test]
fn nonlethal_character_damage_selects_reaction_and_tracks_injury_edges() {
    let mut world = World::new();
    let source = world.spawn();
    let target = world.spawn();
    let _ = world.insert(target, Health::new(100.0));
    let _ = world.insert(target, crate::gameplay::CharacterLifeState::Alive);
    let _ = world.insert(target, DamageReceiver::character());
    let _ = world.insert(
        target,
        CharacterDamageResponseTuning {
            stagger_damage_fraction: 0.50,
            stagger_impulse_threshold: 10.0,
            flinch_duration_seconds: 0.20,
            stagger_duration_seconds: 0.50,
            injured_health_fraction: 0.80,
        },
    );

    let flinch = resolve_weapon_impact(
        &mut world,
        WeaponImpact {
            sequence: 1,
            source,
            target,
            base_damage: 5.0,
            point: Vec3::ZERO,
            normal: Vec3::Y,
            direction: -Vec3::Z,
            distance: 1.0,
            range: 100.0,
            subshape_id: 0,
            momentum_ns: 1.0,
            ammo_impulse_multiplier: 1.0,
            falloff_multiplier: 1.0,
        },
    )
    .expect("flinch impact");
    assert_eq!(flinch.reaction, CharacterHitReactionKind::Flinch);
    assert!(!flinch.injured);
    assert!(!flinch.lethal);
    let _ = crate::gameplay::drain_gameplay_events(&mut world);

    world
        .get_mut::<CharacterDamageResponseTuning>(target)
        .unwrap()
        .stagger_damage_fraction = 0.10;
    let stagger = resolve_weapon_impact(
        &mut world,
        WeaponImpact {
            sequence: 2,
            base_damage: 20.0,
            ..WeaponImpact {
                sequence: 1,
                source,
                target,
                base_damage: 5.0,
                point: Vec3::ZERO,
                normal: Vec3::Y,
                direction: -Vec3::Z,
                distance: 1.0,
                range: 100.0,
                subshape_id: 0,
                momentum_ns: 1.0,
                ammo_impulse_multiplier: 1.0,
                falloff_multiplier: 1.0,
            }
        },
    )
    .expect("stagger impact");
    assert_eq!(stagger.reaction, CharacterHitReactionKind::Stagger);
    assert!(stagger.injured);
    let reaction = world
        .get::<CharacterHitReactionState>(target)
        .expect("reaction state");
    assert!(reaction.active());
    assert_eq!(reaction.kind, CharacterHitReactionKind::Stagger);
    assert!(world
        .get::<CharacterInjuryState>(target)
        .is_some_and(|state| state.injured));
    let events = crate::gameplay::drain_gameplay_events(&mut world);
    assert!(events
        .iter()
        .any(|event| event.id == crate::gameplay::GAMEPLAY_EVENT_CHARACTER_INJURED));
    assert!(events
        .iter()
        .any(|event| event.id == crate::gameplay::GAMEPLAY_EVENT_CHARACTER_HIT_REACTION));

    update_character_damage_states(&mut world, 0.25);
    update_character_damage_states(&mut world, 0.25);
    assert!(!world
        .get::<CharacterHitReactionState>(target)
        .expect("reaction state")
        .active());
    world.get_mut::<Health>(target).unwrap().heal(100.0);
    assert!(!reconcile_character_injury_state(&mut world, target));
    let events = crate::gameplay::drain_gameplay_events(&mut world);
    assert!(events
        .iter()
        .any(|event| { event.id == crate::gameplay::GAMEPLAY_EVENT_CHARACTER_INJURY_RECOVERED }));
}

#[test]
fn lethal_transition_disables_control_drops_exact_weapon_and_can_become_corpse() {
    use crate::gameplay::{
        equip_item_instance, give_item, select_equipment_slot, spawn_default_player, EquipmentSlot,
        ItemCatalog, ItemDefinition, ItemId, WeaponFireMode,
    };

    let mut world = World::new();
    let source = world.spawn();
    let target = spawn_default_player(&mut world, None, "death-target", Vec3::ZERO);
    let ammo = ItemId::from_name("ammo.death.test").expect("ammo id");
    let weapon_id = ItemId::from_name("weapon.death.test").expect("weapon id");
    let sidearm_id = ItemId::from_name("weapon.death.sidearm.test").expect("sidearm id");
    let ammo_definition = ItemDefinition::stackable(
        "ammo.death.test",
        "Death Test Ammo",
        crate::gameplay::ItemKind::Ammo,
        100,
        0.01,
    )
    .expect("ammo definition");
    let weapon_definition = ItemDefinition::weapon(
        "weapon.death.test",
        "Death Test Weapon",
        EquipmentSlot::Primary,
        crate::gameplay::HitscanWeaponTuning::default(),
        ammo,
        WeaponFireMode::SemiAuto,
        2.0,
    )
    .expect("weapon definition");
    let sidearm_definition = ItemDefinition::weapon(
        "weapon.death.sidearm.test",
        "Death Test Sidearm",
        EquipmentSlot::Sidearm,
        crate::gameplay::HitscanWeaponTuning::default(),
        ammo,
        WeaponFireMode::SemiAuto,
        1.0,
    )
    .expect("sidearm definition");
    let mut catalog = ItemCatalog::default();
    catalog.register(ammo_definition).expect("register ammo");
    catalog
        .register(weapon_definition)
        .expect("register weapon");
    catalog
        .register(sidearm_definition)
        .expect("register sidearm");
    world.insert_resource(catalog);
    let mutation = give_item(&mut world, target, weapon_id, 1).expect("give weapon");
    let instance = mutation.touched_instances[0];
    equip_item_instance(&mut world, target, instance).expect("equip weapon");
    let sidearm_mutation = give_item(&mut world, target, sidearm_id, 1).expect("give sidearm");
    let sidearm_instance = sidearm_mutation.touched_instances[0];
    equip_item_instance(&mut world, target, sidearm_instance).expect("equip sidearm");
    select_equipment_slot(&mut world, target, EquipmentSlot::Primary)
        .expect("select primary before death");
    let _ = world.insert(target, DamageReceiver::character());
    let _ = world.insert(
        target,
        CharacterDeathPolicy {
            drop_active_weapon: true,
            presentation: CharacterDeathPresentation::AnimationThenRagdoll,
        },
    );
    world.get_mut::<Health>(target).unwrap().current = 10.0;

    let resolution = resolve_weapon_impact(
        &mut world,
        WeaponImpact {
            sequence: 77,
            source,
            target,
            base_damage: 50.0,
            point: Vec3::new(1.0, 2.0, 3.0),
            normal: Vec3::Y,
            direction: -Vec3::Z,
            distance: 2.0,
            range: 100.0,
            subshape_id: 0,
            momentum_ns: 5.0,
            ammo_impulse_multiplier: 1.0,
            falloff_multiplier: 1.0,
        },
    )
    .expect("lethal impact");
    assert!(resolution.lethal);
    assert!(world
        .get::<CharacterControlState>(target)
        .is_some_and(|state| !state.enabled));
    assert!(world
        .get::<PlayerController>(target)
        .is_some_and(|controller| !controller.enabled));
    let inventory = world
        .get::<crate::gameplay::PlayerInventory>(target)
        .expect("inventory after death");
    assert!(inventory.entry(instance).is_none());
    assert!(inventory.entry(sidearm_instance).is_some());
    assert_eq!(
        inventory.active_slot, None,
        "death must not auto-select a surviving equipped weapon"
    );
    let death = world
        .get::<CharacterDeathTransitionState>(target)
        .expect("death transition");
    assert_eq!(death.phase, CharacterDeathPhase::TransitionRequested);
    let dropped = death.dropped_weapon_entity.expect("dropped weapon entity");
    assert!(world
        .iter_entities()
        .any(|entity| entity.stable_u64() == dropped));

    assert!(mark_character_corpse(&mut world, target));
    assert_eq!(
        world
            .get::<CharacterDeathTransitionState>(target)
            .expect("corpse transition")
            .phase,
        CharacterDeathPhase::Corpse
    );
    assert!(!mark_character_corpse(&mut world, target));
}

#[test]
fn weapon_impact_without_authored_receiver_is_rejected() {
    let mut world = World::new();
    let source = world.spawn();
    let target = world.spawn();
    let _ = world.insert(target, Health::new(100.0));
    let result = resolve_weapon_impact(
        &mut world,
        WeaponImpact {
            sequence: 2,
            source,
            target,
            base_damage: 20.0,
            point: Vec3::ZERO,
            normal: Vec3::Y,
            direction: -Vec3::Z,
            distance: 5.0,
            range: 100.0,
            subshape_id: 0,
            momentum_ns: 2.0,
            ammo_impulse_multiplier: 1.0,
            falloff_multiplier: 1.0,
        },
    );
    assert!(result.is_none());
    assert!((world.get::<Health>(target).unwrap().current - 100.0).abs() < 1.0e-6);
}
