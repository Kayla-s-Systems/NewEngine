use super::*;
    use crate::gameplay::{
        apply_loadout, apply_player_stance_geometry, inventory_quantity, remove_item,
        spawn_default_player, spawn_persistent_item_pickup, try_collect_item_pickup, CharacterBody,
        EquipmentSlot, InventoryLoadout, InventoryLoadoutCatalog, InventoryLoadoutEntry,
        ItemCatalog, ItemDefinition, ItemId, ItemKind, ItemUseEffect, PlayerStanceKind,
    };
    use newengine_math::Vec3;

    const TEST_AMMO_NAME: &str = "test.snapshot.ammo";
    const TEST_WEAPON_NAME: &str = "test.snapshot.weapon";
    const TEST_MEDKIT_NAME: &str = "test.snapshot.medkit";
    const TEST_LOADOUT_NAME: &str = "test.snapshot.loadout";

    fn item_id(name: &str) -> ItemId {
        ItemId::from_name(name).expect("valid test item")
    }

    fn install_test_content(world: &mut World) {
        let ammo =
            ItemDefinition::stackable(TEST_AMMO_NAME, "Snapshot Ammo", ItemKind::Ammo, 120, 0.01)
                .expect("ammo");
        let weapon = ItemDefinition::weapon(
            TEST_WEAPON_NAME,
            "Snapshot Weapon",
            EquipmentSlot::Primary,
            HitscanWeaponTuning::default(),
            ammo.id,
            crate::gameplay::WeaponFireMode::SemiAuto,
            1.0,
        )
        .expect("weapon");
        let medkit = ItemDefinition::consumable(
            TEST_MEDKIT_NAME,
            "Snapshot Medkit",
            4,
            0.25,
            ItemUseEffect::Heal { amount: 25.0 },
        )
        .expect("medkit");

        let mut catalog = ItemCatalog::default();
        catalog.register(ammo).expect("register ammo");
        catalog.register(weapon).expect("register weapon");
        catalog.register(medkit).expect("register medkit");
        world.insert_resource(catalog);

        let mut loadout = InventoryLoadout::new(TEST_LOADOUT_NAME).expect("loadout");
        loadout.entries = vec![
            InventoryLoadoutEntry {
                item: item_id(TEST_WEAPON_NAME),
                quantity: 1,
                equip: true,
            },
            InventoryLoadoutEntry {
                item: item_id(TEST_AMMO_NAME),
                quantity: 30,
                equip: false,
            },
            InventoryLoadoutEntry {
                item: item_id(TEST_MEDKIT_NAME),
                quantity: 2,
                equip: false,
            },
        ];
        let mut loadouts = InventoryLoadoutCatalog::default();
        loadouts.register(loadout).expect("register loadout");
        world.insert_resource(loadouts);
    }

    #[test]
    fn runtime_snapshot_restores_character_damage_lifecycle_state() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "damage-snapshot-player", Vec3::ZERO);
        let baseline_tuning = crate::gameplay::CharacterDamageResponseTuning {
            stagger_damage_fraction: 0.25,
            stagger_impulse_threshold: 6.0,
            flinch_duration_seconds: 0.2,
            stagger_duration_seconds: 0.5,
            injured_health_fraction: 0.35,
        };
        let _ = world.insert(player, baseline_tuning);
        let before = capture_runtime_world_snapshot(&world);

        world
            .get_mut::<crate::gameplay::CharacterControlState>(player)
            .expect("character control")
            .enabled = false;
        let _ = world.insert(
            player,
            crate::gameplay::CharacterDamageResponseTuning {
                stagger_damage_fraction: 0.9,
                ..baseline_tuning
            },
        );
        let _ = world.insert(
            player,
            crate::gameplay::CharacterInjuryState {
                injured: true,
                revision: 9,
            },
        );
        let _ = world.insert(
            player,
            crate::gameplay::CharacterHitReactionState {
                kind: crate::gameplay::CharacterHitReactionKind::Stagger,
                remaining_seconds: 0.4,
                sequence: 11,
                source: 12,
                hit_zone: Some("torso".to_owned()),
                point: Vec3::new(1.0, 2.0, 3.0),
                impulse: Vec3::new(0.0, 0.0, -3.0),
                applied_damage: 30.0,
                health_fraction: 0.4,
                revision: 4,
            },
        );
        let _ = world.insert(
            player,
            crate::gameplay::CharacterDeathPolicy {
                drop_active_weapon: true,
                presentation: crate::gameplay::CharacterDeathPresentation::Ragdoll,
            },
        );
        let _ = world.insert(
            player,
            crate::gameplay::CharacterDeathTransitionState {
                phase: crate::gameplay::CharacterDeathPhase::TransitionRequested,
                sequence: 13,
                source: 14,
                hit_zone: Some("head".to_owned()),
                point: Vec3::ZERO,
                impulse: -Vec3::Z,
                dropped_weapon_entity: Some(99),
                presentation: crate::gameplay::CharacterDeathPresentation::Ragdoll,
                revision: 2,
            },
        );

        restore_runtime_world_snapshot(&mut world, before);

        assert!(world
            .get::<crate::gameplay::CharacterControlState>(player)
            .is_some_and(|state| state.enabled));
        assert_eq!(
            world
                .get::<crate::gameplay::CharacterDamageResponseTuning>(player)
                .copied(),
            Some(baseline_tuning)
        );
        assert!(world
            .get::<crate::gameplay::CharacterInjuryState>(player)
            .is_none());
        assert!(world
            .get::<crate::gameplay::CharacterHitReactionState>(player)
            .is_none());
        assert!(world
            .get::<crate::gameplay::CharacterDeathPolicy>(player)
            .is_none());
        assert!(world
            .get::<crate::gameplay::CharacterDeathTransitionState>(player)
            .is_none());
    }

    #[test]
    fn runtime_snapshot_restores_stance_bounds_weapon_health_and_stamina_state() {
        let mut world = World::new();
        install_test_content(&mut world);
        let body = CharacterBody::default().sanitized();
        let player = spawn_default_player(
            &mut world,
            None,
            "snapshot-player",
            Vec3::new(0.0, body.standing_half_height + body.radius, 0.0),
        );
        apply_loadout(&mut world, player, item_id(TEST_LOADOUT_NAME)).expect("apply loadout");

        let source_pickup = spawn_persistent_item_pickup(
            &mut world,
            None,
            item_id(TEST_AMMO_NAME),
            4,
            Vec3::new(3.0, 1.0, 0.0),
            "snapshot.pickup.ammo",
            5.0,
        )
        .expect("persistent pickup");
        let medkit = item_id(TEST_MEDKIT_NAME);
        let medkits_before = inventory_quantity(&world, player, medkit);
        world.insert_resource(GameplayModalState::default());
        let before = capture_runtime_world_snapshot(&world);
        let standing_y = world
            .get::<Transform>(player)
            .expect("transform")
            .position
            .y;

        apply_player_stance_geometry(&mut world, player, PlayerStanceKind::Crouched, 5);
        if let Some(weapon) = world.get_mut::<PlayerWeaponState>(player) {
            weapon.ammo_in_magazine = 1;
            weapon.reserve_ammo = 0;
        }
        if let Some(health) = world.get_mut::<Health>(player) {
            health.current = 7.0;
        }
        let _ = world.insert(player, CharacterLifeState::Dead);
        if let Some(stamina) = world.get_mut::<Stamina>(player) {
            stamina.current = 3.0;
            stamina.regen_delay_remaining = 9.0;
            stamina.exhausted = true;
        }
        world.insert_resource(GameplayModalState {
            active: true,
            capture: newengine_input_capture_api::GameplayInputCapture::modal(),
            provider_count: 1,
            revision: 1,
        });
        remove_item(&mut world, player, medkit, medkits_before).expect("remove medkits");
        assert_eq!(inventory_quantity(&world, player, medkit), 0);
        assert!(try_collect_item_pickup(&mut world, player, source_pickup));
        assert_eq!(
            world
                .get::<DisplayVisibility>(source_pickup)
                .expect("dormant visibility")
                .mode,
            crate::gameplay::DisplayMode::RuntimeHidden
        );

        restore_runtime_world_snapshot(&mut world, before);

        assert_eq!(
            world
                .get::<PlayerStanceState>(player)
                .expect("stance")
                .current,
            PlayerStanceKind::Standing
        );
        assert!(
            (world
                .get::<Transform>(player)
                .expect("transform")
                .position
                .y
                - standing_y)
                .abs()
                < 1.0e-6
        );
        assert_eq!(
            world
                .get::<PlayerWeaponState>(player)
                .expect("weapon")
                .ammo_in_magazine,
            HitscanWeaponTuning::default().magazine_capacity
        );
        assert_eq!(world.get::<Health>(player).expect("health").current, 100.0);
        assert_eq!(
            world.get::<CharacterLifeState>(player).copied(),
            Some(CharacterLifeState::Alive)
        );
        let stamina = world.get::<Stamina>(player).expect("stamina");
        assert_eq!(stamina.current, 100.0);
        assert_eq!(stamina.regen_delay_remaining, 0.0);
        assert!(!stamina.exhausted);
        assert_eq!(inventory_quantity(&world, player, medkit), medkits_before);
        assert!(world.get::<EquippedWeaponBinding>(player).is_some());
        assert!(
            !world
                .resource::<GameplayModalState>()
                .expect("gameplay modal state")
                .active
        );
        assert!(world.exists(source_pickup));
        assert_eq!(
            world
                .get::<ItemPickup>(source_pickup)
                .expect("restored pickup")
                .quantity,
            4
        );
        assert_eq!(
            world
                .get::<DisplayVisibility>(source_pickup)
                .expect("restored pickup visibility")
                .mode,
            crate::gameplay::DisplayMode::Both
        );
        assert!(world.get::<PhysicsBodyDesc>(source_pickup).is_some());
        assert!(world.get::<WorldItemPresentation>(source_pickup).is_some());
        assert!(world.get::<WorldItemRuntime>(source_pickup).is_some());
        let bounds = world.get::<Bounds>(player).expect("bounds");
        assert!(
            (bounds.local_aabb.half_extents().y - (body.standing_half_height + body.radius)).abs()
                < 1.0e-6
        );
    }

    #[test]
    fn runtime_snapshot_restores_enemy_ai_foundation_state() {
        let mut world = World::new();
        let enemy = world.spawn();
        let target = world.spawn();
        let _ = world.insert(enemy, CombatTeam::new(2));
        let _ = world.insert(
            enemy,
            AIController {
                enabled: true,
                decision_interval_seconds: 0.15,
                decision_cooldown_remaining: 0.07,
            },
        );
        let _ = world.insert(
            enemy,
            PerceptionTuning {
                sight_range: 28.0,
                field_of_view_degrees: 120.0,
                memory_seconds: 4.0,
            },
        );
        let _ = world.insert(
            enemy,
            PerceptionState {
                candidate_target: Some(target),
                visible_target: Some(target),
                candidate_distance: 6.0,
                observation_revision: 3,
            },
        );
        let _ = world.insert(
            enemy,
            TargetMemory {
                target: Some(target),
                visible: true,
                last_known_position: Vec3::new(1.0, 2.0, 3.0),
                seconds_since_seen: 0.0,
                revision: 4,
            },
        );
        let _ = world.insert(
            enemy,
            CombatIntent {
                kind: crate::gameplay::CombatIntentKind::Engage,
                target: Some(target),
                target_position: Vec3::new(1.0, 2.0, 3.0),
                revision: 5,
            },
        );
        let _ = world.insert(
            enemy,
            AIPerceptionProbe {
                seq: 77,
                target,
                origin: Vec3::ZERO,
                direction: -Vec3::Z,
                max_distance: 6.0,
                sample_dt: 1.0 / 60.0,
            },
        );
        let _ = world.insert(
            enemy,
            AINavigationTuning {
                move_speed: 2.8,
                investigate_arrival_distance: 0.9,
                engage_standoff_distance: 7.5,
                waypoint_arrival_distance: 0.4,
                repath_interval_seconds: 0.3,
                view_turn_speed_radians_per_second: 4.2,
            },
        );
        let _ = world.insert(
            enemy,
            AINavigationState {
                goal: Some(Vec3::new(1.0, 2.0, 3.0)),
                path: vec![Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0)],
                waypoint_index: 1,
                repath_remaining_seconds: 0.2,
                revision: 6,
            },
        );
        let _ = world.insert(
            enemy,
            CombatActuationState {
                aim: true,
                trigger_pressed: true,
                trigger_held: true,
                reload_pressed: false,
                source_frame: 123,
            },
        );

        let snapshot = capture_runtime_world_snapshot(&world);
        let _ = world.remove::<CombatTeam>(enemy);
        let _ = world.remove::<AIController>(enemy);
        let _ = world.remove::<PerceptionTuning>(enemy);
        let _ = world.remove::<PerceptionState>(enemy);
        let _ = world.remove::<TargetMemory>(enemy);
        let _ = world.remove::<CombatIntent>(enemy);
        let _ = world.remove::<AIPerceptionProbe>(enemy);
        let _ = world.remove::<AINavigationTuning>(enemy);
        let _ = world.remove::<AINavigationState>(enemy);
        let _ = world.remove::<CombatActuationState>(enemy);

        restore_runtime_world_snapshot(&mut world, snapshot);

        assert_eq!(
            world.get::<CombatTeam>(enemy).copied(),
            Some(CombatTeam::new(2))
        );
        assert_eq!(
            world.get::<AIController>(enemy).copied(),
            Some(AIController {
                enabled: true,
                decision_interval_seconds: 0.15,
                decision_cooldown_remaining: 0.07,
            })
        );
        assert_eq!(
            world.get::<TargetMemory>(enemy).copied().unwrap().target,
            Some(target)
        );
        assert_eq!(
            world.get::<CombatIntent>(enemy).copied().unwrap().kind,
            crate::gameplay::CombatIntentKind::Engage
        );
        assert_eq!(
            world.get::<AIPerceptionProbe>(enemy).copied().unwrap().seq,
            77
        );
        assert_eq!(
            world.get::<AINavigationTuning>(enemy).unwrap().move_speed,
            2.8
        );
        let navigation = world
            .get::<AINavigationState>(enemy)
            .expect("navigation state");
        assert_eq!(navigation.waypoint_index, 1);
        assert_eq!(navigation.path.len(), 2);
        assert_eq!(
            world.get::<CombatActuationState>(enemy).copied(),
            Some(CombatActuationState {
                aim: true,
                trigger_pressed: true,
                trigger_held: true,
                reload_pressed: false,
                source_frame: 123,
            })
        );
    }
