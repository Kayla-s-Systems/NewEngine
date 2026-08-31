#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{
        drain_interaction_events, drain_weapon_events, inventory_quantity, remove_item,
        select_equipment_slot, spawn_default_player, spawn_persistent_item_pickup, EquipmentSlot,
        EquippedWeaponBinding, GameplayContentProvider, ItemCatalog, ItemDefinition, ItemId,
        ItemPickup, PlayerInventory, PlayerStanceState, WeaponFireMode, WeaponType,
    };
    use newengine_fps_content_runtime::{
        embedded_test_content_provider, embedded_test_policy_provider, ensure_fps_player_loadouts,
    };
    use newengine_gameplay_script_runtime::GameplayCommandExecutor;
    use newengine_math::Quat;
    use newengine_physics_api::PhysicsQueryHitDto;

    fn default_rifle_item_id() -> ItemId {
        ItemId::from_name("weapon.rifle.standard").expect("valid test rifle id")
    }

    fn default_rifle_ammo_id() -> ItemId {
        ItemId::from_name("ammo.rifle.standard").expect("valid test rifle ammo id")
    }

    fn spawn_fps_player(world: &mut World, name: &str, position: Vec3) -> EntityId {
        let content = embedded_test_content_provider();
        GameplayContentProvider::install(&content, world).expect("install FPS content");
        let player = spawn_default_player(world, None, name, position);
        ensure_fps_player_loadouts(world);
        // Unit tests do not run the game-ready presentation provider, so publish a deterministic
        // physical compatibility projection explicitly. Production resolves the authored
        // EquippedWeaponEntity -> WeaponEntitySockets muzzle first.
        let muzzle = EquippedWeaponMuzzle::new(
            position + Vec3::new(0.18, 1.20, -0.62),
            Vec3::new(0.0, 0.0, -1.0),
        )
        .expect("test muzzle");
        let _ = world.insert(player, muzzle);
        player
    }

    #[test]
    fn recoil_kicks_camera_up_instead_of_down() {
        let mut world = World::new();
        let player = world.spawn();
        let _ = world.insert(player, CharacterMotor::default());
        let tuning = HitscanWeaponTuning {
            recoil_pitch_radians: 0.05,
            recoil_yaw_radians: 0.0,
            ..HitscanWeaponTuning::default()
        };
        if let Some(motor) = world.get_mut::<CharacterMotor>(player) {
            motor.pitch = 0.0;
        }

        apply_recoil(
            &mut world,
            player,
            newengine_engine_runtime::gameplay::ItemInstanceId(1),
            tuning,
            false,
            1,
        );

        let motor = world.get::<CharacterMotor>(player).copied().expect("motor");
        assert!(
            motor.pitch > 0.0,
            "recoil must increase pitch to look upward"
        );
        let forward =
            Quat::from_euler(newengine_math::EulerRot::YXZ, motor.yaw, motor.pitch, 0.0) * -Vec3::Z;
        assert!(
            forward.y > 0.0,
            "post-recoil camera forward must point upward"
        );
    }

    #[test]
    fn weapon_fires_reloads_and_applies_typed_damage() {
        let mut world = World::new();
        let shooter = spawn_fps_player(&mut world, "shooter", Vec3::ZERO);
        let target = world.spawn();
        let _ = world.insert(target, Health::new(100.0));
        let _ = world.insert(target, Transform::default());
        let _ = world.insert(shooter, HitscanWeaponTuning::default());
        let _ = world.insert(shooter, PlayerWeaponState::default());
        let firing_binding = active_equipped_weapon_binding(&world, shooter)
            .expect("authoritative firing weapon binding");
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(shooter) {
            commands
                .actions
                .pressed
                .push(fps_action::PLAYER_FIRE_PRIMARY.into());
            commands
                .actions
                .held
                .push(fps_action::PLAYER_FIRE_PRIMARY.into());
        }

        step_player_combat(&mut world, 1.0 / 60.0, 1);
        let pending = world
            .get::<PendingHitscan>(shooter)
            .copied()
            .expect("pending hitscan");
        assert_eq!(pending.weapon_instance_id, firing_binding.instance_id);
        let semantic_events = newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world);
        let fired = semantic_events
            .iter()
            .find(|event| event.id == GAMEPLAY_EVENT_WEAPON_FIRED)
            .expect("semantic weapon fired event");
        assert_eq!(fired.source, Some(shooter.stable_u64()));
        assert_eq!(
            fired.payload.get("shot_sequence").and_then(serde_json::Value::as_u64),
            Some(pending.shot_sequence)
        );
        assert_eq!(
            fired.payload.get("weapon").and_then(serde_json::Value::as_str),
            Some("weapon.rifle.standard")
        );
        assert!(
            fired.payload.get("shot_origin").and_then(serde_json::Value::as_array).is_some(),
            "fired event must expose the authoritative muzzle-originated shot"
        );
        let map = BTreeMap::from([
            (shooter.stable_u64(), shooter),
            (target.stable_u64(), target),
        ]);
        resolve_combat_queries(
            &mut world,
            1,
            &[PhysicsQueryHitDto {
                seq: pending.query_seq,
                entity: target.stable_u64(),
                position: [0.0, 0.0, -2.0],
                normal: [0.0, 0.0, 1.0],
                distance: 2.0,
            }],
            &map,
            embedded_test_policy_provider().as_ref(),
            &GameplayCommandExecutor::default(),
        );

        assert_eq!(world.get::<Health>(target).expect("health").current, 75.0);
        let semantic_events = newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world);
        let hit = semantic_events
            .iter()
            .find(|event| event.id == GAMEPLAY_EVENT_WEAPON_HIT)
            .expect("semantic weapon hit event");
        assert_eq!(hit.source, Some(shooter.stable_u64()));
        assert_eq!(
            hit.payload.get("target").and_then(serde_json::Value::as_u64),
            Some(target.stable_u64())
        );
        let events = drain_weapon_events(&mut world);
        assert!(events.iter().any(|event| {
            event.kind == WeaponEventKind::Fired
                && event.weapon_instance_id == firing_binding.instance_id
        }));
        assert!(events.iter().any(|event| {
            event.kind == WeaponEventKind::Hit
                && event.weapon_instance_id == firing_binding.instance_id
                && event.target == Some(target)
                && event.damage == 25.0
        }));
    }

    #[test]
    fn reload_state_machine_transfers_ammo_after_fixed_duration() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "reload-player", Vec3::ZERO);
        let binding = active_equipped_weapon_binding(&world, player).expect("primary firearm");
        let tuning = binding
            .weapon
            .firearm
            .expect("primary firearm definition")
            .tuning;
        let ammo_item = default_rifle_ammo_id();
        let reserve_before = inventory_quantity(&world, player, ammo_item);
        remove_item(
            &mut world,
            player,
            ammo_item,
            reserve_before.saturating_sub(10),
        )
        .expect("trim inventory ammunition");
        let empty_state = PlayerWeaponState {
            ammo_in_magazine: 0,
            reserve_ammo: 10,
            ..PlayerWeaponState::loaded(tuning)
        };
        world
            .get_mut::<PlayerInventory>(player)
            .expect("player inventory")
            .weapon_states
            .insert(binding.instance_id, empty_state);
        let _ = world.insert(player, empty_state);
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands
                .actions
                .pressed
                .push(fps_action::PLAYER_RELOAD.into());
        }

        step_player_combat(&mut world, 0.01, 1);
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands
                .actions
                .pressed
                .retain(|action| action != fps_action::PLAYER_RELOAD);
        }
        let reload_steps = (tuning.reload_duration / 0.1).ceil() as u64 + 2;
        for tick in 2..=reload_steps + 1 {
            step_player_combat(&mut world, 0.1, tick);
        }

        let state = world
            .get::<PlayerWeaponState>(player)
            .expect("weapon state");
        assert_eq!(state.ammo_in_magazine, 10);
        assert_eq!(state.reserve_ammo, 0);
        assert_eq!(inventory_quantity(&world, player, ammo_item), 0);
        assert_eq!(state.reload_remaining, 0.0);
        let events = drain_weapon_events(&mut world);
        assert!(events
            .iter()
            .any(|event| event.kind == WeaponEventKind::ReloadStarted));
        assert!(events
            .iter()
            .any(|event| event.kind == WeaponEventKind::ReloadCompleted));
    }

    #[test]
    fn empty_inventory_resolves_to_unarmed_melee_without_ads_or_firearm_actions() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "unarmed-player", Vec3::ZERO);
        let target = world.spawn();
        let _ = world.insert(target, Health::new(100.0));
        let _ = world.insert(target, Transform::default());

        // No equipped inventory weapon resolves to the virtual Unarmed weapon context.
        if let Some(inventory) = world.get_mut::<PlayerInventory>(player) {
            inventory.equipped.clear();
            inventory.active_slot = None;
            inventory.weapon_states.clear();
        }
        sync_equipped_weapon_runtime(&mut world, player);
        let binding = active_equipped_weapon_binding(&world, player).expect("unarmed binding");
        assert_eq!(binding.slot, None);
        assert_eq!(binding.weapon.weapon_type, WeaponType::Unarmed);
        assert!(binding.capabilities().melee);
        assert!(!binding.capabilities().aim);
        assert!(!binding.capabilities().fire);
        assert!(!binding.capabilities().reload);
        assert!(world.get::<EquippedWeaponBinding>(player).is_some());
        assert!(world.get::<PlayerWeaponState>(player).is_some());
        assert!(world.get::<HitscanWeaponTuning>(player).is_none());

        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 41;
            commands
                .actions
                .pressed
                .push(fps_action::PLAYER_FIRE_PRIMARY.into());
            commands
                .actions
                .held
                .push(fps_action::PLAYER_FIRE_PRIMARY.into());
            commands.actions.held.push(fps_action::PLAYER_AIM.into());
        }
        step_player_combat(&mut world, 1.0 / 60.0, 20);

        let pending = world
            .get::<PendingHitscan>(player)
            .copied()
            .expect("unarmed melee trace");
        assert_eq!(pending.weapon_instance_id, binding.instance_id);
        assert_eq!(pending.attack_kind, WeaponAttackKind::Melee);
        let state = world
            .get::<PlayerWeaponState>(player)
            .expect("unarmed state");
        assert!(!state.aiming, "Unarmed must never enter ADS");
        assert_eq!(state.ammo_in_magazine, 0);
        assert_eq!(state.reserve_ammo, 0);
        let events = drain_weapon_events(&mut world);
        assert!(events.iter().any(|event| {
            event.kind == WeaponEventKind::MeleeAttacked
                && event.weapon_instance_id == binding.instance_id
        }));
        assert!(!events
            .iter()
            .any(|event| event.kind == WeaponEventKind::Fired));
        assert_eq!(
            world.get::<Health>(target).expect("target health").current,
            100.0
        );
    }

    #[test]
    fn unarmed_attack_is_rejected_when_bound_character_has_no_authored_attack_pose() {
        let mut world = World::new();
        let player =
            spawn_default_player(&mut world, None, "unarmed-animation-unsupported", Vec3::ZERO);
        let unarmed = ItemDefinition::typed_weapon(
            "weapon.unarmed",
            "Unarmed",
            None,
            newengine_engine_runtime::gameplay::WeaponItemDefinition::unarmed(
                WeaponType::Unarmed.default_rank(),
                MeleeWeaponTuning::default(),
            ),
            0.0,
        )
        .expect("unarmed definition");
        let mut catalog = ItemCatalog::default();
        catalog.register(unarmed).expect("register unarmed");
        world.insert_resource(catalog);
        let _ = world.insert(player, PlayerInventory::default());
        sync_equipped_weapon_runtime(&mut world, player);
        let binding = active_equipped_weapon_binding(&world, player).expect("unarmed binding");
        assert_eq!(binding.weapon.weapon_type, WeaponType::Unarmed);
        let _ = world.insert(
            player,
            PlayerAuthoredAnimationCapabilities {
                unarmed_attack: false,
                ..Default::default()
            },
        );

        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands
                .actions
                .pressed
                .push(fps_action::PLAYER_FIRE_PRIMARY.into());
        }
        step_player_combat(&mut world, 1.0 / 60.0, 21);

        assert!(
            world.get::<PendingHitscan>(player).is_none(),
            "unsupported authored unarmed attack must not create a damage query"
        );
        let state = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .expect("unarmed state");
        assert_eq!(state.shot_sequence, 0);
        let events = drain_weapon_events(&mut world);
        assert!(
            !events
                .iter()
                .any(|event| event.kind == WeaponEventKind::MeleeAttacked),
            "unsupported authored unarmed attack must not emit MeleeAttacked"
        );
    }

     #[test]
    fn semi_auto_weapon_fires_once_per_press_not_continuously_while_held() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "semi-auto-player", Vec3::ZERO);
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands
                .actions
                .pressed
                .push(fps_action::PLAYER_FIRE_PRIMARY.into());
            commands
                .actions
                .held
                .push(fps_action::PLAYER_FIRE_PRIMARY.into());
        }
        step_player_combat(&mut world, 1.0 / 60.0, 1);
        let first = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .expect("weapon state");
        assert_eq!(first.shot_sequence, 1);

        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions.pressed.clear();
        }
        // Let the fire interval expire while keeping LMB held: semi-auto must not fire again.
        for tick in 2..24 {
            step_player_combat(&mut world, 1.0 / 60.0, tick);
        }
        let held = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .expect("weapon state");
        assert_eq!(held.shot_sequence, 1);

        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands
                .actions
                .pressed
                .push(fps_action::PLAYER_FIRE_PRIMARY.into());
        }
        step_player_combat(&mut world, 1.0 / 60.0, 24);
        let second = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .expect("weapon state");
        assert_eq!(second.shot_sequence, 2);
    }

    #[test]
    fn automatic_weapon_repeats_while_trigger_is_held() {
        let mut world = World::new();
        let content = embedded_test_content_provider();
        GameplayContentProvider::install(&content, &mut world).expect("install FPS content");
        let player = spawn_default_player(&mut world, None, "auto-player", Vec3::ZERO);

        let ammo = ItemId::from_name("ammo.rifle.standard").expect("ammo id");
        let weapon_id = ItemId::from_name("weapon.auto.test").expect("weapon id");
        let weapon = ItemDefinition::weapon(
            "weapon.auto.test",
            "Automatic Test Weapon",
            EquipmentSlot::Primary,
            HitscanWeaponTuning {
                magazine_capacity: 30,
                fire_interval: 0.02,
                ..HitscanWeaponTuning::default()
            },
            ammo,
            WeaponFireMode::Automatic,
            3.0,
        )
        .expect("weapon definition");
        world
            .resource_mut::<ItemCatalog>()
            .expect("catalog")
            .register(weapon)
            .expect("register auto weapon");
        newengine_engine_runtime::gameplay::give_item(&mut world, player, weapon_id, 1)
            .expect("give weapon");
        newengine_engine_runtime::gameplay::give_item(&mut world, player, ammo, 30)
            .expect("give ammo");
        newengine_engine_runtime::gameplay::equip_first_item(&mut world, player, weapon_id)
            .expect("equip auto weapon");

        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands
                .actions
                .held
                .push(fps_action::PLAYER_FIRE_PRIMARY.into());
        }
        for tick in 1..=8 {
            step_player_combat(&mut world, 0.02, tick);
        }
        let state = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .expect("weapon state");
        assert!(
            state.shot_sequence >= 4,
            "automatic trigger should repeat, state={state:?}"
        );
    }

    #[test]
    fn reload_works_for_sidearm_and_consumes_its_own_ammo_type() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "sidearm-reload-player", Vec3::ZERO);
        select_equipment_slot(&mut world, player, EquipmentSlot::Sidearm).expect("select sidearm");
        sync_equipped_weapon_runtime(&mut world, player);
        let binding = world
            .get::<EquippedWeaponBinding>(player)
            .copied()
            .expect("sidearm binding");
        assert_eq!(binding.slot, Some(EquipmentSlot::Sidearm));
        let tuning = world
            .get::<HitscanWeaponTuning>(player)
            .copied()
            .expect("sidearm tuning");
        let ammo_item = binding
            .weapon
            .firearm
            .expect("sidearm firearm definition")
            .ammo_item;
        let reserve_before = inventory_quantity(&world, player, ammo_item);
        let empty_state = PlayerWeaponState {
            ammo_in_magazine: 0,
            reserve_ammo: reserve_before,
            ..PlayerWeaponState::loaded(tuning)
        };
        world
            .get_mut::<PlayerInventory>(player)
            .expect("player inventory")
            .weapon_states
            .insert(binding.instance_id, empty_state);
        let _ = world.insert(player, empty_state);
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands
                .actions
                .pressed
                .push(fps_action::PLAYER_RELOAD.into());
        }
        step_player_combat(&mut world, 0.01, 1);
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions.pressed.clear();
        }
        let reload_steps = (tuning.reload_duration / 0.1).ceil() as u64 + 2;
        for tick in 2..=reload_steps + 1 {
            step_player_combat(&mut world, 0.1, tick);
        }
        let state = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .expect("weapon state");
        let expected = tuning.magazine_capacity.min(reserve_before);
        assert_eq!(state.ammo_in_magazine, expected);
        assert_eq!(
            inventory_quantity(&world, player, ammo_item),
            reserve_before - expected
        );
    }

    #[test]
    fn weapon_switch_does_not_persist_ads_between_instances() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "weapon-switch-ads", Vec3::ZERO);
        let first = active_equipped_weapon_binding(&world, player).expect("primary weapon");

        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions.held.push(fps_action::PLAYER_AIM.into());
        }
        step_player_combat(&mut world, 1.0 / 60.0, 1);
        assert!(world
            .get::<PlayerWeaponState>(player)
            .is_some_and(|state| state.aiming));

        select_equipment_slot(&mut world, player, EquipmentSlot::Sidearm).expect("select sidearm");
        sync_equipped_weapon_runtime(&mut world, player);
        let second = active_equipped_weapon_binding(&world, player).expect("sidearm weapon");
        assert_ne!(first.instance_id, second.instance_id);
        assert!(
            !world
                .get::<PlayerWeaponState>(player)
                .expect("sidearm state")
                .aiming
        );
        let stored_first = world
            .get::<PlayerInventory>(player)
            .and_then(|inventory| inventory.weapon_states.get(&first.instance_id))
            .copied()
            .expect("stored primary state");
        assert!(
            !stored_first.aiming,
            "ADS is transient and must not persist per weapon"
        );
    }

    #[test]
    fn nearby_item_pickup_is_focused_and_collected_without_thin_ray_hit() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "pickup-focus-player", Vec3::ZERO);
        let rifle = default_rifle_item_id();
        while inventory_quantity(&world, player, rifle) > 0 {
            remove_item(&mut world, player, rifle, 1).expect("remove default rifle");
        }
        let _ = world.remove::<EquippedWeaponBinding>(player);
        let _ = world.remove::<PlayerWeaponState>(player);

        let pickup = spawn_persistent_item_pickup(
            &mut world,
            None,
            rifle,
            1,
            Vec3::new(0.8, 0.0, 0.4),
            "test.rifle.focus",
            0.0,
        )
        .expect("spawn focused rifle pickup");
        world
            .get_mut::<ItemPickup>(pickup)
            .expect("pickup")
            .auto_equip = true;

        assert_eq!(focused_item_pickup(&world, player), Some(pickup));
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 19;
            commands
                .actions
                .pressed
                .push(fps_action::PLAYER_INTERACT.into());
        }
        step_player_combat(&mut world, 1.0 / 60.0, 10);
        assert!(world.get::<PendingFocusedItemInteraction>(player).is_some());
        assert!(world.get::<PendingInteraction>(player).is_none());

        let map = BTreeMap::from([(player.stable_u64(), player), (pickup.stable_u64(), pickup)]);
        resolve_combat_queries(
            &mut world,
            10,
            &[],
            &map,
            embedded_test_policy_provider().as_ref(),
            &GameplayCommandExecutor::default(),
        );

        assert_eq!(inventory_quantity(&world, player, rifle), 1);
        let binding = world
            .get::<EquippedWeaponBinding>(player)
            .expect("equipped rifle");
        assert_eq!(binding.item, rifle);
        assert_eq!(binding.slot, Some(EquipmentSlot::Primary));
        assert!(!world.get::<ItemPickup>(pickup).expect("pickup").enabled);
        let events = drain_interaction_events(&mut world);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target, pickup);
    }

    #[test]
    fn interaction_query_emits_typed_target_event() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "player", Vec3::ZERO);
        let target = world.spawn();
        let _ = world.insert(target, Interactable::new("Open terminal"));
        let _ = world.insert(target, Transform::default());
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 7;
            commands
                .actions
                .pressed
                .push(fps_action::PLAYER_INTERACT.into());
        }
        let _ = world.insert(player, PlayerStanceState::standing(0.72));

        step_player_combat(&mut world, 1.0 / 60.0, 2);
        let pending = world
            .get::<PendingInteraction>(player)
            .copied()
            .expect("pending interaction");
        let map = BTreeMap::from([(player.stable_u64(), player), (target.stable_u64(), target)]);
        resolve_combat_queries(
            &mut world,
            2,
            &[PhysicsQueryHitDto {
                seq: pending.query_seq,
                entity: target.stable_u64(),
                position: [0.0, 0.7, -1.0],
                normal: [0.0, 0.0, 1.0],
                distance: 1.0,
            }],
            &map,
            embedded_test_policy_provider().as_ref(),
            &GameplayCommandExecutor::default(),
        );

        let events = drain_interaction_events(&mut world);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].player, player);
        assert_eq!(events[0].target, target);
        assert_eq!(events[0].prompt, "Open terminal");
    }

    #[test]
    fn weapon_obstruction_probe_clamps_safe_muzzle_before_wall() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "obstruction-player", Vec3::ZERO);
        let muzzle =
            EquippedWeaponMuzzle::new(Vec3::new(0.0, 1.20, -0.72), Vec3::new(0.0, 0.0, -1.0))
                .expect("muzzle");
        let _ = world.insert(player, muzzle);
        let _ = world.insert(player, PlayerStanceState::standing(0.72));

        queue_weapon_obstruction_probe(&mut world, player, 44);
        let pending = world
            .get::<PendingWeaponObstructionProbe>(player)
            .copied()
            .expect("obstruction probe");
        let queries = collect_combat_queries(&world);
        let query = queries
            .iter()
            .find(|query| query.seq == pending.query_seq)
            .expect("physics query");
        assert_eq!(query.ignore_entity, Some(player.stable_u64()));

        let wall = world.spawn();
        let hit_distance = pending.muzzle_distance * 0.55;
        let hit_position = pending.origin + pending.direction * hit_distance;
        let map = BTreeMap::from([(player.stable_u64(), player), (wall.stable_u64(), wall)]);
        resolve_combat_queries(
            &mut world,
            44,
            &[PhysicsQueryHitDto {
                seq: pending.query_seq,
                entity: wall.stable_u64(),
                position: [hit_position.x, hit_position.y, hit_position.z],
                normal: [0.0, 0.0, 1.0],
                distance: hit_distance,
            }],
            &map,
            embedded_test_policy_provider().as_ref(),
            &GameplayCommandExecutor::default(),
        );

        let obstruction = world
            .get::<WeaponObstructionState>(player)
            .copied()
            .expect("obstruction state");
        assert!(obstruction.blocked);
        assert!(obstruction.alpha > 0.0);
        let safe_distance = (obstruction.safe_muzzle_position - pending.origin).length();
        assert!(safe_distance < hit_distance);
        assert!(hit_distance - safe_distance >= 0.024);
        assert!(world.get::<PendingWeaponObstructionProbe>(player).is_none());
    }

    #[test]
    fn clear_weapon_obstruction_probe_restores_real_muzzle() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "clear-obstruction-player", Vec3::ZERO);
        let muzzle =
            EquippedWeaponMuzzle::new(Vec3::new(0.0, 1.20, -0.64), Vec3::new(0.0, 0.0, -1.0))
                .expect("muzzle");
        let _ = world.insert(player, muzzle);
        let _ = world.insert(player, PlayerStanceState::standing(0.72));
        queue_weapon_obstruction_probe(&mut world, player, 45);
        let pending = world
            .get::<PendingWeaponObstructionProbe>(player)
            .copied()
            .expect("obstruction probe");

        resolve_combat_queries(
            &mut world,
            45,
            &[],
            &BTreeMap::new(),
            embedded_test_policy_provider().as_ref(),
            &GameplayCommandExecutor::default(),
        );
        let obstruction = world
            .get::<WeaponObstructionState>(player)
            .copied()
            .expect("clear obstruction state");
        assert!(!obstruction.blocked);
        assert_eq!(obstruction.alpha, 0.0);
        assert!((obstruction.safe_muzzle_position - pending.muzzle_position).length() < 1.0e-6);
    }
}

#[test]
fn hitscan_direction_tracks_mouse_look_pitch() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let mut motor = CharacterMotor::default();
    motor.yaw = 0.61;
    motor.pitch = -0.37;
    let _ = world.insert(player, motor);
    let _ = world.insert(
        player,
        PlayerStanceState {
            current_eye_height: 1.62,
            ..PlayerStanceState::default()
        },
    );
    let muzzle =
        EquippedWeaponMuzzle::new(Vec3::new(0.2, 1.3, -0.55), Vec3::new(0.0, 0.0, -1.0))
            .expect("physical muzzle");
    let _ = world.insert(player, muzzle);
    let mut tuning = HitscanWeaponTuning::default();
    tuning.hip_spread_radians = 0.0;
    tuning.aim_spread_radians = 0.0;

    let (_, direction) =
        shot_origin_and_direction(&world, player, tuning, true, 1).expect("view-aligned hitscan");
    let expected = (Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0) * -Vec3::Z)
        .normalize_or_zero();
    assert!(direction.dot(expected) > 0.999_999);
    assert!(direction.y.abs() > 0.1, "pitch must affect shot direction");
}

#[test]
fn hitscan_rejects_camera_only_fire_without_a_physical_muzzle() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(player, CharacterMotor::default());
    let _ = world.insert(player, PlayerStanceState::standing(0.72));
    let mut tuning = HitscanWeaponTuning::default();
    tuning.hip_spread_radians = 0.0;
    tuning.aim_spread_radians = 0.0;

    assert!(
        shot_origin_and_direction(&world, player, tuning, true, 1).is_none(),
        "camera/view state must never synthesize a firearm origin when no physical muzzle exists"
    );
}

#[test]
fn hitscan_origin_is_physical_muzzle_while_direction_converges_to_view_axis() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(player, CharacterMotor::default());
    let _ = world.insert(player, PlayerStanceState::standing(0.72));
    let muzzle = EquippedWeaponMuzzle::new(Vec3::new(0.35, 1.25, -0.65), Vec3::new(0.0, 0.0, -1.0))
        .expect("valid muzzle");
    let _ = world.insert(player, muzzle);
    let mut tuning = HitscanWeaponTuning::default();
    tuning.hip_spread_radians = 0.0;
    tuning.aim_spread_radians = 0.0;

    let (origin, direction) =
        shot_origin_and_direction(&world, player, tuning, true, 9).expect("muzzle hitscan");
    assert!((origin - (muzzle.position + muzzle.forward * 0.008)).length() < 1.0e-6);
    let view_origin = Vec3::Y * 0.72;
    let camera_forward = -Vec3::Z;
    let target = view_origin + camera_forward * tuning.range.min(80.0).max(12.0);
    let expected = (target - origin).normalize_or_zero();
    assert!(direction.dot(expected) > 0.999_999);
    assert!(
        direction.x < 0.0,
        "off-axis muzzle must converge toward reticle axis"
    );
}

#[test]
fn blocked_hitscan_uses_safe_muzzle_on_player_side_of_obstacle() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(player, CharacterMotor::default());
    let _ = world.insert(player, PlayerStanceState::standing(0.72));
    let muzzle = EquippedWeaponMuzzle::new(Vec3::new(0.0, 1.20, -0.75), Vec3::new(0.0, 0.0, -1.0))
        .expect("valid muzzle");
    let _ = world.insert(player, muzzle);
    let safe = Vec3::new(0.0, 1.15, -0.31);
    let _ = world.insert(
        player,
        WeaponObstructionState {
            blocked: true,
            alpha: 0.8,
            hit_position: Vec3::new(0.0, 1.15, -0.34),
            hit_normal: Vec3::Z,
            safe_muzzle_position: safe,
            fixed_tick: 7,
        },
    );
    let mut tuning = HitscanWeaponTuning::default();
    tuning.hip_spread_radians = 0.0;
    tuning.aim_spread_radians = 0.0;
    let (origin, _) =
        shot_origin_and_direction(&world, player, tuning, true, 10).expect("blocked hitscan");
    assert!((origin - safe).length() < 1.0e-6);
    assert!(
        origin.z > -0.34,
        "shot must originate on player side of wall"
    );
}

#[test]
fn interaction_ray_tracks_mouse_look_pitch() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let mut motor = CharacterMotor::default();
    motor.yaw = -0.42;
    motor.pitch = 0.29;
    let _ = world.insert(player, motor);
    let _ = world.insert(player, PlayerStanceState::standing(0.72));

    let (_, direction) = interaction_ray(&world, player, PlayerInteractionTuning::default())
        .expect("view-aligned interaction ray");
    let expected = (Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0) * -Vec3::Z)
        .normalize_or_zero();
    assert!(direction.dot(expected) > 0.999_999);
}
