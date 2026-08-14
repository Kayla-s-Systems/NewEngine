#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{
        embedded_test_content_provider, embedded_test_policy_provider, ensure_fps_player_loadouts,
    };
    use crate::default_rifle_ammo_id;
    use newengine_engine_runtime::gameplay::{
        drain_interaction_events, drain_weapon_events, inventory_quantity, remove_item,
        spawn_default_player, GameplayContentProvider, PlayerStanceState,
    };
    use newengine_math::Quat;
    use newengine_gameplay_script_runtime::GameplayCommandExecutor;
    use newengine_physics_api::PhysicsQueryHitDto;

    fn spawn_fps_player(world: &mut World, name: &str, position: Vec3) -> EntityId {
        let content = embedded_test_content_provider();
        GameplayContentProvider::install(&content, world).expect("install FPS content");
        let player = spawn_default_player(world, None, name, position);
        ensure_fps_player_loadouts(world);
        player
    }

    #[test]
    fn recoil_kicks_camera_up_instead_of_down() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "recoil-player", Vec3::ZERO);
        let tuning = HitscanWeaponTuning {
            recoil_pitch_radians: 0.05,
            recoil_yaw_radians: 0.0,
            ..HitscanWeaponTuning::default()
        };
        if let Some(motor) = world.get_mut::<CharacterMotor>(player) {
            motor.pitch = 0.0;
        }

        apply_recoil(&mut world, player, tuning, 1);

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
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(shooter) {
            commands.actions.held.push(fps_action::PLAYER_FIRE_PRIMARY.into());
        }

        step_player_combat(&mut world, 1.0 / 60.0, 1);
        let pending = world
            .get::<PendingHitscan>(shooter)
            .copied()
            .expect("pending hitscan");
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
        let events = drain_weapon_events(&mut world);
        assert!(events
            .iter()
            .any(|event| event.kind == WeaponEventKind::Fired));
        assert!(events.iter().any(|event| {
            event.kind == WeaponEventKind::Hit
                && event.target == Some(target)
                && event.damage == 25.0
        }));
    }

    #[test]
    fn reload_state_machine_transfers_ammo_after_fixed_duration() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "reload-player", Vec3::ZERO);
        let tuning = HitscanWeaponTuning {
            reload_duration: 0.02,
            ..HitscanWeaponTuning::default()
        };
        let _ = world.insert(player, tuning);
        let ammo_item = default_rifle_ammo_id();
        let reserve_before = inventory_quantity(&world, player, ammo_item);
        remove_item(
            &mut world,
            player,
            ammo_item,
            reserve_before.saturating_sub(10),
        )
        .expect("trim inventory ammunition");
        let _ = world.insert(
            player,
            PlayerWeaponState {
                ammo_in_magazine: 0,
                reserve_ammo: 10,
                ..PlayerWeaponState::loaded(tuning)
            },
        );
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions.pressed.push(fps_action::PLAYER_RELOAD.into());
        }

        step_player_combat(&mut world, 0.01, 1);
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions.pressed.retain(|action| action != fps_action::PLAYER_RELOAD);
        }
        step_player_combat(&mut world, 0.02, 2);

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
    fn interaction_query_emits_typed_target_event() {
        let mut world = World::new();
        let player = spawn_fps_player(&mut world, "player", Vec3::ZERO);
        let target = world.spawn();
        let _ = world.insert(target, Interactable::new("Open terminal"));
        let _ = world.insert(target, Transform::default());
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 7;
            commands.actions.pressed.push(fps_action::PLAYER_INTERACT.into());
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
}
