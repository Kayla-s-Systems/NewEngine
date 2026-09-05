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
        embedded_test_content_provider, ensure_fps_player_loadouts,
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

    pub(super) fn spawn_fps_player(world: &mut World, name: &str, position: Vec3) -> EntityId {
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

        let initial_pitch = motor.pitch;
        recover_weapon_recoil(&mut world, player, 1.0 / 60.0);
        let followed = world.get::<CharacterMotor>(player).copied().expect("motor");
        assert!(
            followed.pitch >= initial_pitch,
            "NorthStar-style recoil tracker must carry a short post-shot follow-through before recovery"
        );
        for _ in 0..45 {
            recover_weapon_recoil(&mut world, player, 1.0 / 60.0);
        }
        let settled = world.get::<CharacterMotor>(player).copied().expect("motor");
        assert!(
            settled.pitch.abs() < initial_pitch * 0.10,
            "camera recoil tracker must settle without leaving a permanent authored offset: initial={} settled={}",
            initial_pitch,
            settled.pitch
        );
    }

    #[test]
    fn authored_recoil_tracker_speed_controls_post_shot_follow_through() {
        fn pitch_after_frame(scale: f32) -> (f32, f32) {
            let mut world = World::new();
            let player = world.spawn();
            let _ = world.insert(player, CharacterMotor::default());
            let tuning = HitscanWeaponTuning {
                recoil_pitch_radians: 0.05,
                recoil_pitch_random_radians: 0.0,
                recoil_yaw_radians: 0.0,
                recoil_yaw_bias_radians: 0.0,
                recoil_recovery_hz: 5.0,
                recoil_pitch_tracker_speed_scale: scale,
                recoil_yaw_tracker_speed_scale: 0.0,
                ..HitscanWeaponTuning::default()
            };
            apply_recoil(
                &mut world,
                player,
                newengine_engine_runtime::gameplay::ItemInstanceId(9),
                tuning,
                false,
                1,
            );
            let initial = world.get::<CharacterMotor>(player).unwrap().pitch;
            recover_weapon_recoil(&mut world, player, 1.0 / 60.0);
            let after = world.get::<CharacterMotor>(player).unwrap().pitch;
            (initial, after)
        }

        let (initial_no_follow, no_follow) = pitch_after_frame(0.0);
        let (initial_strong, strong_follow) = pitch_after_frame(1.8);
        assert!((initial_no_follow - initial_strong).abs() < 1.0e-6);
        assert!(
            no_follow < initial_no_follow,
            "zero tracker speed should enter recovery immediately"
        );
        assert!(
            strong_follow > initial_strong,
            "authored tracker speed must produce measurable post-shot follow-through: initial={initial_strong} after={strong_follow}"
        );
    }

    #[cfg(test)]
    #[test]
    fn resolved_weapon_stat_stack_modulates_runtime_recoil() {
        fn pitch_after_kick(recoil_multiplier: f32) -> f32 {
            let mut world = World::new();
            let player = world.spawn();
            let _ = world.insert(player, CharacterMotor::default());
            let _ = world.insert(
                player,
                newengine_engine_runtime::gameplay::WeaponStatModifierStack {
                    modifiers: vec![
                        newengine_engine_runtime::gameplay::WeaponStatModifier::multiply(
                            newengine_engine_runtime::gameplay::WeaponStatId::RecoilMultiplier,
                            recoil_multiplier,
                        ),
                    ],
                },
            );
            let tuning = HitscanWeaponTuning {
                recoil_pitch_radians: 0.05,
                recoil_pitch_random_radians: 0.0,
                recoil_yaw_radians: 0.0,
                recoil_yaw_bias_radians: 0.0,
                ..HitscanWeaponTuning::default()
            };
            apply_recoil(
                &mut world,
                player,
                newengine_engine_runtime::gameplay::ItemInstanceId(77),
                tuning,
                false,
                1,
            );
            world
                .get::<CharacterMotor>(player)
                .expect("character motor")
                .pitch
        }

        let normal = pitch_after_kick(1.0);
        let suppressed = pitch_after_kick(0.0);
        let doubled = pitch_after_kick(2.0);

        assert!(normal > 0.0, "baseline recoil must kick upward");
        assert!(
            suppressed.abs() < 1.0e-7,
            "resolved stat stack must be able to suppress recoil: {suppressed}"
        );
        assert!(
            (doubled - normal * 2.0).abs() < 1.0e-6,
            "resolved recoil multiplier must reach runtime exactly once: normal={normal} doubled={doubled}"
        );
    }

    #[test]
    fn ai_engage_uses_shared_weapon_hitscan_pipeline_without_direct_damage() {
        let mut world = World::new();
        let content = embedded_test_content_provider();
        GameplayContentProvider::install(&content, &mut world).expect("install FPS content");

        let actor = world.spawn();
        let target = world.spawn();
        let _ = world.insert(
            actor,
            Transform {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        let _ = world.insert(actor, CharacterBody::default());
        let _ = world.insert(actor, CharacterMotor::default());
        let _ = world.insert(actor, AIController::default());
        let _ = world.insert(actor, CharacterControlState::enabled());
        let _ = world.insert(actor, CharacterLifeState::Alive);
        let _ = world.insert(actor, Health::new(100.0));
        let _ = world.insert(
            actor,
            PerceptionState {
                candidate_target: Some(target),
                visible_target: Some(target),
                candidate_distance: 5.0,
                observation_revision: 1,
            },
        );
        let _ = world.insert(
            actor,
            CombatIntent {
                kind: CombatIntentKind::Engage,
                target: Some(target),
                target_position: Vec3::new(0.0, 0.0, -5.0),
                revision: 1,
            },
        );
        let _ = world.insert(
            actor,
            FpsAiCombatTuning {
                fire_distance: 20.0,
                aim_tolerance_radians: 4.0_f32.to_radians(),
            },
        );
        let _ = world.insert(
            actor,
            EquippedWeaponMuzzle::new(Vec3::new(0.18, 1.20, -0.62), Vec3::new(0.0, 0.0, -1.0))
                .expect("AI test muzzle"),
        );
        newengine_engine_runtime::gameplay::apply_loadout(
            &mut world,
            actor,
            ItemId::from_name("loadout.fps.default").expect("test loadout id"),
        )
        .expect("apply authored AI test loadout");

        let _ = world.insert(
            target,
            Transform {
                position: Vec3::new(0.0, 0.0, -5.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        let _ = world.insert(target, CharacterBody::default());
        let _ = world.insert(target, Health::new(100.0));
        let _ = world.insert(
            target,
            newengine_engine_runtime::gameplay::DamageReceiver::character(),
        );

        assert!(world.get::<PlayerController>(actor).is_none());
        step_ai_combat_actuation(&mut world, 41);
        let actuation = world
            .get::<CombatActuationState>(actor)
            .copied()
            .expect("AI combat actuation");
        assert!(actuation.aim);
        assert!(actuation.trigger_pressed);
        assert!(actuation.trigger_held);
        assert_eq!(world.get::<Health>(target).unwrap().current, 100.0);

        step_actor_combat(&mut world, 1.0 / 60.0, 41);
        let pending = world
            .get::<PendingHitscan>(actor)
            .copied()
            .expect("AI must enter the ordinary pending hitscan pipeline");
        assert_eq!(world.get::<Health>(target).unwrap().current, 100.0);

        let map = BTreeMap::from([(actor.stable_u64(), actor), (target.stable_u64(), target)]);
        resolve_combat_queries(
            &mut world,
            41,
            &[PhysicsQueryHitDto {
                subshape_id: 0,
                hit_index: 0,
                back_face: false,
                seq: pending.query_seq,
                entity: target.stable_u64(),
                position: [0.0, 0.9, -5.0],
                normal: [0.0, 0.0, 1.0],
                distance: 5.0,
            }],
            &map,
            newengine_fps_content_runtime::embedded_test_policy_provider().as_ref(),
            &GameplayCommandExecutor::default(),
        );
        assert!(
            world.get::<Health>(target).unwrap().current < 100.0,
            "damage must occur only after ordinary combat query resolution"
        );
    }

    #[test]
    fn weapon_fires_reloads_and_applies_typed_damage() {
        let mut world = World::new();
        let shooter = spawn_fps_player(&mut world, "shooter", Vec3::ZERO);
        let target = world.spawn();
        let _ = world.insert(target, Health::new(100.0));
        let _ = world.insert(
            target,
            newengine_engine_runtime::gameplay::DamageReceiver::character(),
        );
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
        assert_eq!(
            world
                .get::<WeaponActionRuntime>(shooter)
                .copied()
                .unwrap()
                .action,
            WeaponActionKind::Firing
        );
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
            fired
                .payload
                .get("shot_sequence")
                .and_then(serde_json::Value::as_u64),
            Some(pending.shot_sequence)
        );
        assert_eq!(
            fired
                .payload
                .get("weapon")
                .and_then(serde_json::Value::as_str),
            Some("weapon.rifle.standard")
        );
        assert!(
            fired
                .payload
                .get("shot_origin")
                .and_then(serde_json::Value::as_array)
                .is_some(),
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
                subshape_id: 0,
                hit_index: 0,
                back_face: false,

                seq: pending.query_seq,
                entity: target.stable_u64(),
                position: [0.0, 0.0, -2.0],
                normal: [0.0, 0.0, 1.0],
                distance: 2.0,
            }],
            &map,
            newengine_fps_content_runtime::embedded_test_policy_provider().as_ref(),
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
            hit.payload
                .get("target")
                .and_then(serde_json::Value::as_u64),
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
    fn reload_ammo_moves_only_at_authored_commit_phase_and_publishes_semantics() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "reload-commit-phase", Vec3::ZERO);
        let binding = active_equipped_weapon_binding(&world, player).expect("primary firearm");
        let firearm = binding.weapon.firearm.expect("firearm definition");
        let tuning = firearm.tuning;
        let timeline = firearm.profiles.sanitized().handling.reload_timeline;
        let ammo_item = firearm.ammo_item;

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

        let _ = drain_weapon_events(&mut world);
        let _ = newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world);
        let _ = newengine_engine_runtime::gameplay::drain_animation_semantic_events(&mut world);

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

        let action = world
            .get::<WeaponActionRuntime>(player)
            .copied()
            .expect("reload action");
        assert_eq!(action.action, WeaponActionKind::Reloading);
        let commit_time = action.duration_seconds * timeline.ammo_commit_fraction;
        assert!(commit_time > 0.0 && commit_time < action.duration_seconds);

        let mut tick = 2_u64;
        loop {
            let action = world
                .get::<WeaponActionRuntime>(player)
                .copied()
                .expect("reload action");
            if action.elapsed_seconds + 0.1 >= commit_time {
                break;
            }
            step_player_combat(&mut world, 0.1, tick);
            tick += 1;
            let state = world
                .get::<PlayerWeaponState>(player)
                .copied()
                .expect("weapon state");
            assert_eq!(
                state.ammo_in_magazine, 0,
                "magazine must stay empty before AmmoCommitted"
            );
            assert_eq!(
                inventory_quantity(&world, player, ammo_item),
                10,
                "reserve inventory must not change before AmmoCommitted"
            );
        }

        let _ = drain_weapon_events(&mut world);
        let _ = newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world);
        let _ = newengine_engine_runtime::gameplay::drain_animation_semantic_events(&mut world);

        step_player_combat(&mut world, 0.1, tick);

        let state = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .expect("weapon state");
        assert_eq!(state.ammo_in_magazine, 10);
        assert_eq!(state.reserve_ammo, 0);
        assert_eq!(inventory_quantity(&world, player, ammo_item), 0);

        let weapon_events = drain_weapon_events(&mut world);
        let kinds = weapon_events
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>();
        assert!(
            kinds.contains(&WeaponEventKind::ReloadAmmoCommitted),
            "crossing authored commit threshold must emit ReloadAmmoCommitted: {kinds:?}"
        );
        assert!(
            !kinds.contains(&WeaponEventKind::ReloadCompleted),
            "ammo commit must occur before reload completion"
        );

        let semantic_events = newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world);
        let commit_semantic = semantic_events
            .iter()
            .find(|event| {
                event.id == GAMEPLAY_EVENT_WEAPON_RELOAD_PHASE
                    && event
                        .payload
                        .get("reload_phase")
                        .and_then(serde_json::Value::as_str)
                        == Some("ammo_committed")
            })
            .expect("reload phase semantic event");
        assert_eq!(commit_semantic.source, Some(player.stable_u64()));

        let animation_events =
            newengine_engine_runtime::gameplay::drain_animation_semantic_events(&mut world);
        assert!(
            animation_events
                .iter()
                .any(|event| event.event == "character.weapon.firearm.ammo_committed"),
            "AmmoCommitted must drive a dedicated animation pulse"
        );
    }

    #[test]
    fn animation_authority_blocks_fallback_commit_until_clip_markers_arrive() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "reload-animation-authority", Vec3::ZERO);
        let binding = active_equipped_weapon_binding(&world, player).expect("primary firearm");
        let firearm = binding.weapon.firearm.expect("firearm definition");
        let tuning = firearm.tuning;
        let ammo_item = firearm.ammo_item;

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
        let _ = world.insert(
            player,
            WeaponReloadAnimationAuthority {
                weapon_instance_id: binding.instance_id,
                clip_duration_seconds: 0.6,
                marker_mask: newengine_engine_runtime::gameplay::WEAPON_RELOAD_ANIMATION_REQUIRED_MARKER_MASK,
                required_marker_mask: newengine_engine_runtime::gameplay::WEAPON_RELOAD_ANIMATION_REQUIRED_MARKER_MASK,
            },
        );

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

        let action = world
            .get::<WeaponActionRuntime>(player)
            .copied()
            .expect("reload action");
        assert_eq!(action.action, WeaponActionKind::Reloading);
        assert_eq!(
            action.timing_source,
            WeaponActionTimingSource::AnimationMarkers
        );
        assert!(
            (action.duration_seconds - firearm.profiles.handling.reload_duration_seconds).abs()
                < 1.0e-6,
            "marker authority owns semantic phase commits, not the gameplay action clock"
        );

        for tick in 2..=12 {
            step_player_combat(&mut world, 0.1, tick);
        }
        let stalled = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .expect("weapon state");
        assert_eq!(
            stalled.ammo_in_magazine, 0,
            "animation authority must suppress percentage AmmoCommitted even beyond clip duration"
        );
        assert_eq!(inventory_quantity(&world, player, ammo_item), 10);
        assert_eq!(
            world
                .get::<WeaponActionRuntime>(player)
                .copied()
                .expect("reload action")
                .action,
            WeaponActionKind::Reloading,
            "elapsed time alone must not finish marker-authoritative reload"
        );

        for phase in [
            WeaponReloadPhase::MagazineDetached,
            WeaponReloadPhase::AmmoCommitted,
        ] {
            newengine_engine_runtime::gameplay::queue_weapon_reload_animation_marker(
                &mut world,
                player,
                WeaponReloadAnimationMarker {
                    weapon_instance_id: binding.instance_id,
                    phase,
                    clip_time_seconds: 0.3,
                    playback_time_seconds: 0.3,
                    loop_index: 0,
                },
            );
        }
        let _ = drain_weapon_events(&mut world);
        step_player_combat(&mut world, 0.01, 13);

        let committed = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .expect("weapon state");
        assert_eq!(committed.ammo_in_magazine, 10);
        assert_eq!(committed.reserve_ammo, 0);
        assert_eq!(inventory_quantity(&world, player, ammo_item), 0);
        let events = drain_weapon_events(&mut world);
        assert!(events
            .iter()
            .any(|event| event.kind == WeaponEventKind::ReloadAmmoCommitted));
        assert!(!events
            .iter()
            .any(|event| event.kind == WeaponEventKind::ReloadCompleted));

        for phase in [
            WeaponReloadPhase::MagazineInserted,
            WeaponReloadPhase::Chambered,
            WeaponReloadPhase::Complete,
        ] {
            newengine_engine_runtime::gameplay::queue_weapon_reload_animation_marker(
                &mut world,
                player,
                WeaponReloadAnimationMarker {
                    weapon_instance_id: binding.instance_id,
                    phase,
                    clip_time_seconds: 0.6,
                    playback_time_seconds: 0.6,
                    loop_index: 0,
                },
            );
        }
        step_player_combat(&mut world, 0.01, 14);

        let completed = world
            .get::<WeaponActionRuntime>(player)
            .copied()
            .expect("weapon action");
        assert_eq!(completed.action, WeaponActionKind::Ready);
        assert_eq!(completed.reload_phase, WeaponReloadPhase::Complete);
        assert_eq!(
            world
                .get::<PlayerWeaponState>(player)
                .copied()
                .expect("weapon state")
                .reload_remaining,
            0.0
        );
        let events = drain_weapon_events(&mut world);
        assert!(events
            .iter()
            .any(|event| event.kind == WeaponEventKind::ReloadCompleted));
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
        assert_eq!(
            world
                .get::<WeaponActionRuntime>(player)
                .copied()
                .unwrap()
                .action,
            WeaponActionKind::Melee
        );

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
        let player = spawn_default_player(
            &mut world,
            None,
            "unarmed-animation-unsupported",
            Vec3::ZERO,
        );
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
        let muzzle =
            EquippedWeaponMuzzle::new(Vec3::new(0.18, 1.20, -0.62), Vec3::new(0.0, 0.0, -1.0))
                .expect("auto muzzle");
        let _ = world.insert(player, muzzle);

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
            newengine_fps_content_runtime::embedded_test_policy_provider().as_ref(),
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
                subshape_id: 0,
                hit_index: 0,
                back_face: false,

                seq: pending.query_seq,
                entity: target.stable_u64(),
                position: [0.0, 0.7, -1.0],
                normal: [0.0, 0.0, 1.0],
                distance: 1.0,
            }],
            &map,
            newengine_fps_content_runtime::embedded_test_policy_provider().as_ref(),
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
                subshape_id: 0,
                hit_index: 0,
                back_face: false,

                seq: pending.query_seq,
                entity: wall.stable_u64(),
                position: [hit_position.x, hit_position.y, hit_position.z],
                normal: [0.0, 0.0, 1.0],
                distance: hit_distance,
            }],
            &map,
            newengine_fps_content_runtime::embedded_test_policy_provider().as_ref(),
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
            newengine_fps_content_runtime::embedded_test_policy_provider().as_ref(),
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
    let motor = CharacterMotor {
        yaw: 0.61,
        pitch: -0.37,
        ..Default::default()
    };
    let _ = world.insert(player, motor);
    let _ = world.insert(
        player,
        PlayerStanceState {
            current_eye_height: 1.62,
            ..PlayerStanceState::default()
        },
    );
    let muzzle = EquippedWeaponMuzzle::new(Vec3::new(0.2, 1.3, -0.55), Vec3::new(0.0, 0.0, -1.0))
        .expect("physical muzzle");
    let _ = world.insert(player, muzzle);
    let tuning = HitscanWeaponTuning {
        hip_spread_radians: 0.0,
        aim_spread_radians: 0.0,
        ..Default::default()
    };

    let (origin, direction) =
        shot_origin_and_direction(&world, player, tuning, true, 1).expect("view-aligned hitscan");
    let camera_forward = (Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0) * -Vec3::Z)
        .normalize_or_zero();
    let view_origin = Vec3::Y * 1.62;
    let aim_point = view_origin + camera_forward * tuning.ads_center_screen_convergence_m();
    let expected = (aim_point - origin).normalize_or_zero();
    assert!(direction.dot(expected) > 0.999_999);
    assert!(
        direction.y.abs() > 0.1,
        "pitch must affect the converged ballistic direction"
    );
    assert!(
        (direction - camera_forward).length() > 1.0e-5,
        "off-axis muzzle must converge toward the view axis rather than pretending the bullet originated at the camera"
    );
}

#[test]
fn ads_hitscan_follows_rendered_ironsight_line_from_physical_muzzle() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let motor = CharacterMotor {
        yaw: 0.35,
        pitch: -0.18,
        ..Default::default()
    };
    let _ = world.insert(player, motor);
    let _ = world.insert(player, PlayerStanceState::standing(0.72));
    let muzzle = EquippedWeaponMuzzle::new(Vec3::new(0.28, 1.24, -0.61), Vec3::new(0.0, 0.0, -1.0))
        .expect("physical muzzle");
    let _ = world.insert(player, muzzle);

    // Deliberately publish an off-axis visual sight. ADS ballistics must follow what the player
    // actually sees through the weapon rather than silently using a different camera-only ray.
    let rendered_sight_forward = Vec3::new(-0.22, 0.17, -0.96).normalize_or_zero();
    let sight = EquippedWeaponSight::new(Vec3::new(0.21, 1.34, -0.42), rendered_sight_forward)
        .expect("rendered sight");
    let _ = world.insert(player, sight);
    let tuning = HitscanWeaponTuning {
        hip_spread_radians: 0.0,
        aim_spread_radians: 0.0,
        ..Default::default()
    };

    let (origin, direction) = shot_origin_and_direction(&world, player, tuning, true, 77)
        .expect("center-screen ADS hitscan");
    assert!((origin - muzzle.position).length() < 1.0e-6);

    let aim_point =
        sight.position + rendered_sight_forward * tuning.ads_center_screen_convergence_m();
    let expected = (aim_point - origin).normalize_or_zero();
    assert!(
        direction.dot(expected) > 0.999_999,
        "ADS bullet must originate at the physical muzzle and converge on the visible iron-sight line"
    );
}

#[test]
fn hitscan_rejects_camera_only_fire_without_a_physical_muzzle() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(player, CharacterMotor::default());
    let _ = world.insert(player, PlayerStanceState::standing(0.72));
    let tuning = HitscanWeaponTuning {
        hip_spread_radians: 0.0,
        aim_spread_radians: 0.0,
        ..Default::default()
    };

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
    let tuning = HitscanWeaponTuning {
        hip_spread_radians: 0.0,
        aim_spread_radians: 0.0,
        ..Default::default()
    };

    let (origin, direction) =
        shot_origin_and_direction(&world, player, tuning, true, 9).expect("muzzle hitscan");
    assert!((origin - muzzle.position).length() < 1.0e-6);
    let view_origin = Vec3::Y * 0.72;
    let camera_forward = -Vec3::Z;
    let target = view_origin + camera_forward * tuning.ads_center_screen_convergence_m();
    let expected = (target - origin).normalize_or_zero();
    assert!(direction.dot(expected) > 0.999_999);
    assert!(
        direction.x < 0.0,
        "off-axis muzzle must converge toward reticle axis"
    );
}

#[test]
fn blocked_hitscan_keeps_exact_rendered_muzzle_origin() {
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
    let tuning = HitscanWeaponTuning {
        hip_spread_radians: 0.0,
        aim_spread_radians: 0.0,
        ..Default::default()
    };
    let (origin, _) =
        shot_origin_and_direction(&world, player, tuning, true, 10).expect("blocked hitscan");
    assert!((origin - muzzle.position).length() < 1.0e-6);
    assert!(
        (origin - safe).length() > 1.0e-3,
        "obstruction state must not relocate the physical shot origin away from the rendered muzzle"
    );
}

#[test]
fn interaction_ray_tracks_mouse_look_pitch() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let motor = CharacterMotor {
        yaw: -0.42,
        pitch: 0.29,
        ..Default::default()
    };
    let _ = world.insert(player, motor);
    let _ = world.insert(player, PlayerStanceState::standing(0.72));

    let (_, direction) = interaction_ray(&world, player, PlayerInteractionTuning::default())
        .expect("view-aligned interaction ray");
    let expected = (Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0) * -Vec3::Z)
        .normalize_or_zero();
    assert!(direction.dot(expected) > 0.999_999);
}

fn test_ballistics() -> BallisticShotProfile {
    BallisticShotProfile {
        projectile_mass_kg: 0.004,
        muzzle_velocity_mps: 800.0,
        momentum_ns: 3.2,
        remaining_penetration_energy_j: 1200.0,
        max_penetration_m: 0.50,
        damage_multiplier: 1.0,
        impulse_multiplier: 1.0,
        falloff_start_m: 0.0,
        falloff_end_m: 100.0,
        falloff_min_multiplier: 1.0,
        component_falloff_multiplier: 1.0,
    }
}

#[test]
fn instant_projectile_ricochets_once_from_grazing_metal_surface() {
    let mut world = World::new();
    let shooter = world.spawn();
    let plate = world.spawn();
    let _ = world.insert(
        plate,
        PhysicsSurface {
            id: "surface.metal.sheet".to_owned(),
            ..PhysicsSurface::default()
        },
    );
    let _ = world.insert(
        plate,
        BallisticMaterialResponse {
            penetration_resistance_j_per_m: 20000.0,
            entry_energy_cost_j: 2000.0,
            damage_transfer_multiplier: 1.0,
            impulse_transfer_multiplier: 1.0,
            ricochet_allowed: true,
            ricochet_max_incidence_dot: 0.38,
            ricochet_energy_retention: 0.38,
        },
    );
    let pending = PendingHitscan {
        query_seq: hitscan_query_seq(shooter, 9),
        weapon_instance_id: ItemInstanceId(17),
        attack_kind: WeaponAttackKind::Firearm,
        shot_sequence: 9,
        origin: Vec3::ZERO,
        direction: Vec3::new(0.96, 0.0, -0.28).normalize(),
        range: 100.0,
        damage: 25.0,
        ballistics: test_ballistics(),
        bounce_count: 0,
        max_bounces: 1,
        ricochet_grazing_dot: 0.38,
        ricochet_energy_retention: 0.38,
    };
    let _ = world.insert(shooter, pending);
    let map = BTreeMap::from([(shooter.stable_u64(), shooter), (plate.stable_u64(), plate)]);
    resolve_combat_queries(
        &mut world,
        77,
        &[PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: pending.query_seq,
            entity: plate.stable_u64(),
            position: [3.0, 0.0, -0.875],
            normal: [0.0, 0.0, 1.0],
            distance: 3.125,
        }],
        &map,
        newengine_fps_content_runtime::embedded_test_policy_provider().as_ref(),
        &GameplayCommandExecutor::default(),
    );
    let bounced = world
        .get::<PendingHitscan>(shooter)
        .copied()
        .expect("grazing metal impact must schedule a bounded ricochet collision trace");
    assert_eq!(bounced.bounce_count, 1);
    assert_eq!(bounced.max_bounces, 1);
    assert!(
        bounced.direction.z > 0.0,
        "reflected trace must leave the contact plane"
    );
    assert!((bounced.damage - 9.5).abs() < 1.0e-4);
    assert!(bounced.range < pending.range && bounced.range > 30.0);
    assert_ne!(bounced.query_seq, pending.query_seq);
}

#[test]
fn instant_projectile_does_not_ricochet_from_head_on_or_soft_surface() {
    let firearm = PendingHitscan {
        query_seq: 1,
        weapon_instance_id: ItemInstanceId(1),
        attack_kind: WeaponAttackKind::Firearm,
        shot_sequence: 1,
        origin: Vec3::ZERO,
        direction: -Vec3::Z,
        range: 100.0,
        damage: 25.0,
        ballistics: test_ballistics(),
        bounce_count: 0,
        max_bounces: 1,
        ricochet_grazing_dot: 0.38,
        ricochet_energy_retention: 0.38,
    };
    let material = BallisticMaterialResponse {
        penetration_resistance_j_per_m: 20_000.0,
        entry_energy_cost_j: 2_000.0,
        damage_transfer_multiplier: 1.0,
        impulse_transfer_multiplier: 1.0,
        ricochet_allowed: true,
        ricochet_max_incidence_dot: 0.38,
        ricochet_energy_retention: 0.38,
    };
    assert!(!ballistic_material_allows_ricochet(
        material,
        firearm.direction,
        Vec3::Z,
        firearm.bounce_count,
        firearm.max_bounces,
    ));
    let grazing = PendingHitscan {
        direction: Vec3::new(0.96, 0.0, -0.28).normalize(),
        ..firearm
    };
    let soft = BallisticMaterialResponse {
        ricochet_allowed: false,
        ..material
    };
    assert!(!ballistic_material_allows_ricochet(
        soft,
        grazing.direction,
        Vec3::Z,
        grazing.bounce_count,
        grazing.max_bounces,
    ));
}

#[test]
fn ballistic_ray_penetrates_authored_thickness_and_hits_next_receiver() {
    let mut world = World::new();
    let shooter = world.spawn();
    let wall = world.spawn();
    let target = world.spawn();
    let pending = PendingHitscan {
        query_seq: hitscan_query_seq(shooter, 41),
        weapon_instance_id: ItemInstanceId(41),
        attack_kind: WeaponAttackKind::Firearm,
        shot_sequence: 41,
        origin: Vec3::ZERO,
        direction: -Vec3::Z,
        range: 100.0,
        damage: 30.0,
        ballistics: test_ballistics(),
        bounce_count: 0,
        max_bounces: 0,
        ricochet_grazing_dot: 0.0,
        ricochet_energy_retention: 0.0,
    };
    let _ = world.insert(shooter, pending);
    let _ = world.insert(
        wall,
        BallisticMaterialResponse {
            penetration_resistance_j_per_m: 500.0,
            entry_energy_cost_j: 20.0,
            damage_transfer_multiplier: 1.0,
            impulse_transfer_multiplier: 1.0,
            ricochet_allowed: false,
            ricochet_max_incidence_dot: 0.0,
            ricochet_energy_retention: 0.0,
        },
    );
    let _ = world.insert(
        wall,
        PhysicsSurface {
            id: "surface.concrete.test".to_owned(),
            ..PhysicsSurface::default()
        },
    );
    let _ = world.insert(target, Health::new(100.0));
    let _ = world.insert(
        target,
        newengine_engine_runtime::gameplay::DamageReceiver::character(),
    );
    let map = BTreeMap::from([
        (shooter.stable_u64(), shooter),
        (wall.stable_u64(), wall),
        (target.stable_u64(), target),
    ]);
    let hits = [
        PhysicsQueryHitDto {
            seq: pending.query_seq,
            entity: wall.stable_u64(),
            subshape_id: 3,
            hit_index: 0,
            back_face: false,
            position: [0.0, 0.0, -2.0],
            normal: [0.0, 0.0, 1.0],
            distance: 2.0,
        },
        PhysicsQueryHitDto {
            seq: pending.query_seq,
            entity: wall.stable_u64(),
            subshape_id: 3,
            hit_index: 1,
            back_face: true,
            position: [0.0, 0.0, -2.1],
            normal: [0.0, 0.0, -1.0],
            distance: 2.1,
        },
        PhysicsQueryHitDto {
            seq: pending.query_seq,
            entity: target.stable_u64(),
            subshape_id: 0,
            hit_index: 2,
            back_face: false,
            position: [0.0, 0.0, -4.0],
            normal: [0.0, 0.0, 1.0],
            distance: 4.0,
        },
    ];
    resolve_combat_queries(
        &mut world,
        41,
        &hits,
        &map,
        newengine_fps_content_runtime::embedded_test_policy_provider().as_ref(),
        &GameplayCommandExecutor::default(),
    );
    assert!(world.get::<Health>(target).expect("target health").current < 100.0);
    let semantic_events = newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world);
    let penetration = semantic_events
        .iter()
        .find(|event| event.id == GAMEPLAY_EVENT_WEAPON_PENETRATED)
        .expect("successful entry/exit traversal must publish penetration semantics");
    assert_eq!(
        penetration
            .payload
            .get("surface")
            .and_then(serde_json::Value::as_str),
        Some("surface.concrete.test")
    );
    assert!(
        (penetration
            .payload
            .get("thickness_m")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or_default()
            - 0.1)
            .abs()
            < 1.0e-4
    );
    let exit_point = penetration
        .payload
        .get("exit_point")
        .and_then(serde_json::Value::as_array)
        .expect("penetration event exit_point");
    assert_eq!(exit_point.len(), 3);
    assert!((exit_point[0].as_f64().unwrap_or(f64::NAN) - 0.0).abs() < 1.0e-6);
    assert!((exit_point[1].as_f64().unwrap_or(f64::NAN) - 0.0).abs() < 1.0e-6);
    assert!((exit_point[2].as_f64().unwrap_or(f64::NAN) + 2.1).abs() < 1.0e-5);
    assert!(
        world.get::<PendingHitscan>(shooter).is_none(),
        "non-ricochet traversal must complete"
    );
}

#[test]
fn weapon_accuracy_accumulates_per_shot_and_recovers_after_authored_delay() {
    let mut world = World::new();
    let player = tests::spawn_fps_player(&mut world, "accuracy-player", Vec3::ZERO);
    let binding = active_equipped_weapon_binding(&world, player).expect("weapon binding");
    let tuning = binding.weapon.firearm.expect("firearm").tuning.sanitized();
    for _ in 0..4 {
        runtime::kick_weapon_accuracy(&mut world, player, binding.instance_id, tuning);
    }
    let peak = world
        .get::<WeaponAccuracyState>(player)
        .copied()
        .expect("accuracy state");
    assert_eq!(peak.shot_count, 4);
    assert!(peak.bloom_radians > 0.0);
    assert!(peak.bloom_radians <= tuning.recoil_accuracy_max_radians + 1.0e-6);
    runtime::recover_weapon_accuracy(
        &mut world,
        player,
        tuning.accuracy_recovery_delay_seconds * 0.5,
    );
    let delayed = world
        .get::<WeaponAccuracyState>(player)
        .copied()
        .expect("delayed state");
    assert!(delayed.bloom_radians >= peak.bloom_radians * 0.99);
    for _ in 0..240 {
        runtime::recover_weapon_accuracy(&mut world, player, 1.0 / 60.0);
    }
    let settled = world
        .get::<WeaponAccuracyState>(player)
        .copied()
        .expect("settled state");
    assert!(
        settled.bloom_radians < peak.bloom_radians * 0.1,
        "bloom must recover independently of camera recoil"
    );
}

#[test]
fn firing_pattern_state_machine_distinguishes_semi_auto_and_burst() {
    let mut world = World::new();
    let player = world.spawn();
    let instance = ItemInstanceId(99);
    let semi = FiringPatternDefinition::from_fire_mode(
        newengine_engine_runtime::gameplay::WeaponFireMode::SemiAuto,
        0.1,
    );
    assert!(runtime::fire_controller_wants_shot(
        &mut world,
        player,
        instance,
        semi,
        FpsActionFrame {
            fire_primary_pressed: true,
            fire_primary_held: true,
            ..Default::default()
        },
        0.016,
    ));
    runtime::fire_controller_commit_shot(&mut world, player, instance, semi);
    assert!(!runtime::fire_controller_wants_shot(
        &mut world,
        player,
        instance,
        semi,
        FpsActionFrame {
            fire_primary_held: true,
            ..Default::default()
        },
        0.016,
    ));
    let _ = world.remove::<WeaponFireControllerState>(player);
    let automatic = FiringPatternDefinition::from_fire_mode(
        newengine_engine_runtime::gameplay::WeaponFireMode::Automatic,
        0.1,
    );
    assert!(runtime::fire_controller_wants_shot(
        &mut world,
        player,
        instance,
        automatic,
        FpsActionFrame {
            fire_primary_held: true,
            ..Default::default()
        },
        0.016,
    ));
    let _ = world.remove::<WeaponFireControllerState>(player);
    let burst = FiringPatternDefinition {
        kind: FiringPatternKind::Burst,
        shots_per_burst_min: 3,
        shots_per_burst_max: 3,
        burst_cooldown: 0.25,
        ..FiringPatternDefinition::default()
    };
    let press = FpsActionFrame {
        fire_primary_pressed: true,
        fire_primary_held: true,
        ..Default::default()
    };
    let held = FpsActionFrame {
        fire_primary_held: true,
        ..Default::default()
    };
    assert!(runtime::fire_controller_wants_shot(
        &mut world, player, instance, burst, press, 0.0,
    ));
    runtime::fire_controller_commit_shot(&mut world, player, instance, burst);
    assert!(runtime::fire_controller_wants_shot(
        &mut world, player, instance, burst, held, 0.0,
    ));
    runtime::fire_controller_commit_shot(&mut world, player, instance, burst);
    assert!(runtime::fire_controller_wants_shot(
        &mut world, player, instance, burst, held, 0.0,
    ));
    runtime::fire_controller_commit_shot(&mut world, player, instance, burst);
    assert!(!runtime::fire_controller_wants_shot(
        &mut world, player, instance, burst, held, 0.0,
    ));
}
