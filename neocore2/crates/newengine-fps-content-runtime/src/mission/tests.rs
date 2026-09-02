#[cfg(test)]
mod world_item_runtime_tests {
    use super::*;

    #[test]
    fn skeletal_character_presentation_does_not_inherit_capsule_scale() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let capsule_scale = Vec3::new(0.55, 1.05, 0.55);
        let _ = world.insert(
            entity,
            Transform {
                position: Vec3::new(1.0, 2.0, 3.0),
                rotation: Quat::IDENTITY,
                scale: capsule_scale,
            },
        );
        let body = newengine_engine_runtime::gameplay::CharacterBody {
            radius: 0.55,
            standing_half_height: 0.5,
            crouched_half_height: 0.5,
            standing_eye_height: 0.5,
            crouched_eye_height: 0.5,
            visual_radius: 0.55,
            visual_half_height: 1.05,
        }
        .sanitized();
        let _ = world.insert(entity, body);

        normalize_character_actor_presentation_basis(&mut world, entity);

        assert_eq!(world.get::<Transform>(entity).unwrap().scale, Vec3::ONE);
        let preserved = *world
            .get::<newengine_engine_runtime::gameplay::CharacterBody>(entity)
            .expect("character body");
        assert!((preserved.radius - 0.55).abs() <= f32::EPSILON);
        assert!((preserved.visual_half_height - 1.05).abs() <= f32::EPSILON);
    }

    #[test]
    fn authored_ai_target_composes_shared_character_damage_and_ai_foundation() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let target = AuthoredMissionTargetSpec {
            id: "dummy.enemy.test".to_owned(),
            character_ref: None,
            position: Vec3::ZERO,
            health: 125.0,
            scale: Vec3::new(0.55, 1.05, 0.55),
            ai: Some(crate::authored_world_profile::GameReadyEnemyAiSpec {
                combat_team: 2,
                sight_range: 24.0,
                field_of_view_degrees: 110.0,
                memory_seconds: 3.0,
                decision_interval_seconds: 0.1,
                navigation: newengine_engine_runtime::gameplay::AINavigationTuning {
                    move_speed: 2.4,
                    investigate_arrival_distance: 0.8,
                    engage_standoff_distance: 8.0,
                    waypoint_arrival_distance: 0.35,
                    repath_interval_seconds: 0.35,
                    view_turn_speed_radians_per_second: 240.0_f32.to_radians(),
                },
                patrol_route: vec![Vec3::new(-2.0, 0.0, -4.0), Vec3::new(2.0, 0.0, -4.0)],
                patrol_looping: true,
                combat: newengine_gameplay_fps_api::FpsAiCombatTuning {
                    fire_distance: 22.0,
                    aim_tolerance_radians: 3.0_f32.to_radians(),
                },
                weapon_mount: newengine_gameplay_fps_api::FpsActorWeaponMountTuning {
                    local_offset: [0.20, 1.20, -0.45],
                    local_forward: [0.0, 0.0, -1.0],
                },
                loadout: "loadout.fps.default".to_owned(),
            }),
        };

        attach_enemy_character_foundation(&mut world, entity, &target);

        assert!(world
            .get::<newengine_engine_runtime::gameplay::GameplayActor>(entity)
            .is_some());
        assert!(world
            .get::<newengine_engine_runtime::gameplay::CharacterBody>(entity)
            .is_some());
        assert!(world.get::<newengine_sim::CharacterMotor>(entity).is_some());
        assert!(world
            .get::<newengine_engine_runtime::gameplay::CharacterControlState>(entity)
            .is_some_and(|state| state.enabled));
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::Health>(entity)
                .expect("health")
                .current,
            125.0
        );
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::CharacterLifeState>(entity)
                .copied(),
            Some(newengine_engine_runtime::gameplay::CharacterLifeState::Alive)
        );
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::DamageReceiver>(entity)
                .expect("damage receiver")
                .kind,
            newengine_engine_runtime::gameplay::DamageReceiverKind::Character
        );
        assert!(world
            .get::<newengine_engine_runtime::gameplay::DamageHitZoneMap>(entity)
            .is_some());
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::CombatTeam>(entity)
                .copied(),
            Some(newengine_engine_runtime::gameplay::CombatTeam::new(2))
        );
        assert!(world
            .get::<newengine_engine_runtime::gameplay::AIController>(entity)
            .is_some_and(|controller| controller.enabled));
        let perception = world
            .get::<newengine_engine_runtime::gameplay::PerceptionTuning>(entity)
            .expect("perception tuning");
        assert_eq!(perception.sight_range, 24.0);
        assert_eq!(perception.field_of_view_degrees, 110.0);
        assert_eq!(perception.memory_seconds, 3.0);
        assert!(world
            .get::<newengine_engine_runtime::gameplay::TargetMemory>(entity)
            .is_some());
        assert!(world
            .get::<newengine_engine_runtime::gameplay::CombatIntent>(entity)
            .is_some());
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::PhysicsBodyDesc>(entity)
                .expect("dynamic enemy physics")
                .kind,
            newengine_physics_contracts::PhysicsBodyKind::Dynamic
        );
        assert!(world
            .get::<newengine_engine_runtime::gameplay::AINavigationState>(entity)
            .is_some());
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::AINavigationTuning>(entity)
                .expect("navigation tuning")
                .move_speed,
            2.4
        );
        let patrol = world
            .get::<newengine_engine_runtime::gameplay::AIPatrolRoute>(entity)
            .expect("patrol route");
        assert_eq!(patrol.waypoints.len(), 2);
        assert!(patrol.looping);
        assert!(world
            .get::<newengine_engine_runtime::gameplay::AIPatrolState>(entity)
            .is_some());
        assert_eq!(
            world
                .get::<newengine_gameplay_fps_api::FpsAiCombatTuning>(entity)
                .expect("AI combat tuning")
                .fire_distance,
            22.0
        );
        assert_eq!(
            world
                .get::<newengine_gameplay_fps_api::FpsActorWeaponMountTuning>(entity)
                .expect("AI weapon mount")
                .local_offset,
            [0.20, 1.20, -0.45]
        );
        assert_eq!(
            world
                .get::<newengine_gameplay_fps_api::FpsActorLoadoutRequest>(entity)
                .expect("authored enemy loadout request")
                .loadout,
            "loadout.fps.default"
        );
        assert!(world
            .get::<newengine_engine_runtime::gameplay::PlayerController>(entity)
            .is_none());
    }

    #[test]
    fn world_item_material_library_is_scoped_by_mesh_slot() {
        assert_eq!(
            world_item_material_asset(Some("shared/materials/weapon_rifle.nemat"), "m00", None,)
                .as_deref(),
            Some("shared/materials/weapon_rifle.nemat@m00")
        );
        assert_eq!(
            world_item_material_asset(
                Some("shared/materials/weapon_rifle.nemat@m01"),
                "m00",
                None,
            )
            .as_deref(),
            Some("shared/materials/weapon_rifle.nemat@m01")
        );
        assert_eq!(
            world_item_material_asset(None, "m01", Some("shared/materials/weapon_rifle.nemat"))
                .as_deref(),
            Some("shared/materials/weapon_rifle.nemat@m01")
        );
    }

    #[test]
    fn dropped_rifle_physics_uses_canonical_ydd_bounds_not_pickup_box() {
        let min = Vec3::new(-0.069_917_45, -0.065_805_55, -0.372_692_38);
        let max = Vec3::new(0.120_714_34, 0.127_575_71, 0.633_752_35);
        let half = scaled_world_item_half_extents(min, max, Vec3::ONE).expect("rifle bounds");
        assert!((half.x - 0.095_315_9).abs() < 1.0e-5, "half={half:?}");
        assert!((half.y - 0.096_690_63).abs() < 1.0e-5, "half={half:?}");
        assert!((half.z - 0.503_222_35).abs() < 1.0e-5, "half={half:?}");
        assert!(
            half.z > half.x * 5.0,
            "rifle collider must stay elongated: {half:?}"
        );
    }

    #[test]
    fn authored_world_item_render_path_casts_and_receives_shadows() {
        let options = world_item_render_options();
        assert_eq!(
            options.shadow_policy,
            newengine_model_domain_api::MeshShadowPolicy::CastAndReceive
        );
    }
}
