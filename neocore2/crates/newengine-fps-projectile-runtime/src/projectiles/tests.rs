#[cfg(test)]
mod tests {
    use super::*;
    use newengine_gameplay_fps_api::action as fps_action;
    use newengine_input_actions_api::ActionCommandFrame;
    use newengine_scene::SceneState;
    use newengine_sim::CameraRigComp;

    fn setup_player_and_camera(world: &mut World) -> (EntityId, EntityId) {
        world.insert_resource(newengine_game_data::GameDataSnapshot::new(
            "test.project",
            newengine_game_data::GameData::default(),
        ));
        let player = world.spawn();
        let _ = world.insert(player, PlayerController::default());
        let _ = world.insert(player, PlayerCommandFrame::default());
        let _ = world.insert(player, Transform::default());

        let camera = world.spawn();
        let rotation = Quat::from_rotation_y(-0.5) * Quat::from_rotation_x(0.25);
        let _ = world.insert(
            camera,
            CameraRigComp(newengine_camera::CameraRig {
                position: Vec3::new(2.0, 3.0, 4.0),
                rotation,
            }),
        );
        world.insert_resource(SceneState::new(None, Some(camera)));
        (player, camera)
    }

    #[test]
    fn launch_pulse_spawns_one_dynamic_sphere_along_camera_forward() {
        let mut world = World::new();
        let (player, camera) = setup_player_and_camera(&mut world);
        let actions = ActionCommandFrame {
            pressed: vec![fps_action::PLAYER_LAUNCH_PROJECTILE.into()],
            ..ActionCommandFrame::default()
        };
        let _ = world.insert(player, PlayerCommandFrame::new(77, actions));
        let rig = world.get::<CameraRigComp>(camera).copied().unwrap();
        let expected_forward = rig.0.forward().normalize_or_zero();

        step_projectile_sphere_launcher(&mut world, 1.0 / 60.0);

        let projectiles = world
            .query::<ProjectileSphereRuntime>()
            .map(|(entity, runtime)| (entity, *runtime))
            .collect::<Vec<_>>();
        assert_eq!(projectiles.len(), 1);
        let (entity, runtime) = projectiles[0];
        assert_eq!(runtime.owner, player);
        assert_eq!(runtime.source_frame, 77);
        let velocity = world.get::<Velocity>(entity).copied().unwrap().0;
        assert!(velocity.normalize_or_zero().dot(expected_forward) > 0.9999);
        assert!(matches!(
            world.get::<PhysicsBodyDesc>(entity).map(|body| body.shape),
            Some(CollisionShapeDesc::Sphere { .. })
        ));
    }

    #[test]
    fn weapon_shot_fx_starts_at_muzzle_and_stops_at_hitscan_impact() {
        let mut world = World::new();
        let package = newengine_item_assets_runtime::compile_authored_item_package(
            &newengine_item_assets_runtime::test_fps_item_package(),
        )
        .expect("compile test item package");
        newengine_item_assets_runtime::install_compiled_item_package(&mut world, package);
        let (rifle_id, weapon) = {
            let catalog = world.resource::<ItemCatalog>().expect("item catalog");
            let rifle = catalog
                .find("weapon.rifle.standard")
                .expect("test rifle");
            (rifle.id, rifle.weapon.expect("rifle weapon definition"))
        };
        let owner = world.spawn();
        let _ = world.insert(
            owner,
            EquippedWeaponBinding {
                instance_id: newengine_engine_runtime::gameplay::ItemInstanceId(1),
                item: rifle_id,
                slot: Some(newengine_engine_runtime::gameplay::EquipmentSlot::Primary),
                weapon,
            },
        );
        let origin = Vec3::new(1.0, 1.5, 2.0);
        let direction = -Vec3::Z;
        spawn_weapon_shot_fx(&mut world, owner, 17, origin, direction, 120.0);

        let effects = world
            .query::<newengine_vfx_runtime::VfxLayerRuntime>()
            .map(|(entity, runtime)| (entity, *runtime))
            .collect::<Vec<_>>();
        assert_eq!(
            effects.len(),
            3,
            "shot structural layers stay in ECS while smoke is GPU-resident"
        );
        assert_eq!(newengine_vfx_runtime::vfx_runtime_stats(&world).active_layers, 4);
        assert_eq!(
            world
                .resource::<newengine_vfx_api::VfxGpuParticleBridge>()
                .expect("GPU particle bridge")
                .stats()
                .pending_spawns,
            1,
            "shot smoke must be queued for GPU materialization"
        );
        let (tracer, tracer_runtime) = effects
            .iter()
            .copied()
            .find(|(_, runtime)| runtime.kind == newengine_vfx_runtime::VfxLayerKind::Tracer)
            .expect("tracer");
        let tracer_transform = world
            .get::<Transform>(tracer)
            .copied()
            .expect("tracer transform");
        assert!((tracer_runtime.origin - origin).length() < 1.0e-6);
        assert!(tracer_runtime.velocity.normalize_or_zero().dot(direction) > 0.999_999);
        assert!(tracer_transform.position.z < origin.z);
        assert_eq!(
            world.query::<WeaponShellCasing>().count(),
            0,
            "casing must not eject on ignition frame; native slide has not moved yet"
        );
        assert_eq!(
            world.query::<PendingWeaponShellEjection>().count(),
            1,
            "shot must schedule one native frame-1 casing ejection"
        );

        let hit = origin + direction * 1.0;
        let target = world.spawn();
        let _ = world.insert(
            target,
            PhysicsSurface {
                id: "surface.metal".to_owned(),
                ..PhysicsSurface::default()
            },
        );
        resolve_weapon_shot_hit_fx(&mut world, owner, 17, hit, Vec3::Z, Some(target));
        let clamped = world
            .get::<newengine_vfx_runtime::VfxLayerRuntime>(tracer)
            .copied()
            .expect("clamped tracer");
        assert!((clamped.max_distance - 1.0).abs() < 1.0e-6);
        assert_eq!(
            world
                .query::<newengine_vfx_runtime::VfxLayerRuntime>()
                .filter(|(_, runtime)| runtime.kind == newengine_vfx_runtime::VfxLayerKind::ImpactDecal)
                .count(),
            1,
            "authoritative hit must publish one VFX-owned impact decal"
        );
        assert_eq!(
            world
                .query::<newengine_vfx_runtime::VfxLayerRuntime>()
                .filter(|(_, runtime)| runtime.kind == newengine_vfx_runtime::VfxLayerKind::Spark)
                .count(),
            0,
            "spark particles must no longer allocate ECS render entities"
        );
        let gpu_spawns = world
            .resource::<newengine_vfx_api::VfxGpuParticleBridge>()
            .expect("GPU particle bridge")
            .drain_spawns(32);
        assert_eq!(
            gpu_spawns
                .iter()
                .filter(|spawn| spawn.kind == newengine_vfx_api::VfxGpuParticleKind::Spark)
                .count(),
            8,
            "metal impact must publish the deterministic GPU spark burst"
        );

        step_weapon_shot_fx(&mut world, 0.002);
        let after = world
            .get::<Transform>(tracer)
            .copied()
            .expect("travelling tracer");
        let tracer_half_length = tracer_runtime.base_scale.z * 0.5;
        assert!((after.position - origin).length() <= 1.0 + tracer_half_length + 1.0e-4);
        step_weapon_shot_fx(&mut world, 0.01);
        assert!(
            !world.exists(tracer),
            "tracer must terminate at authoritative hit range"
        );
        assert_eq!(
            world.query::<WeaponShellCasing>().count(),
            0,
            "12 ms is still before the recovered frame-1 ejection boundary"
        );
        step_weapon_shot_fx(&mut world, 0.010);
        assert_eq!(world.query::<WeaponShellCasing>().count(), 0);
        step_weapon_shot_fx(&mut world, 0.012);
        let casings = world
            .query::<WeaponShellCasing>()
            .map(|(entity, casing)| (entity, *casing))
            .collect::<Vec<_>>();
        assert_eq!(casings.len(), 1);
        assert_eq!(world.query::<PendingWeaponShellEjection>().count(), 0);
        let (casing, casing_semantic) = casings[0];
        assert_eq!(casing_semantic.owner_stable_id, owner.stable_u64());
        assert_eq!(casing_semantic.shot_sequence, 17);
        assert_eq!(casing_semantic.weapon_item_id, rifle_id.raw());
        assert!(casing_semantic.variant < 5);
        assert!(world
            .get::<Velocity>(casing)
            .is_some_and(|value| value.0.length() > 1.0));
        assert!(world.get::<AngularVelocity>(casing).is_some());
        assert!(matches!(
            world.get::<PhysicsBodyDesc>(casing).map(|body| body.shape),
            Some(CollisionShapeDesc::Box { .. })
        ));
        // Presentation FX may expire, but physical brass is deliberately persistent.
        for _ in 0..30 {
            step_weapon_shot_fx(&mut world, 0.1);
        }
        assert!(
            world.exists(casing),
            "spent casing must remain in the world after settling"
        );
    }

    #[test]
    fn projectile_lifetime_despawns_entity() {
        let mut world = World::new();
        world.insert_resource(newengine_game_data::GameDataSnapshot::new(
            "test.project",
            newengine_game_data::GameData::default(),
        ));
        let owner = world.spawn();
        let entity = spawn_projectile_sphere(
            &mut world,
            owner,
            1,
            Vec3::ZERO,
            -Vec3::Z,
            ProjectileSphereTuning {
                lifetime_seconds: 0.25,
                ..ProjectileSphereTuning::default()
            },
        )
        .unwrap();
        for _ in 0..3 {
            expire_projectile_spheres(&mut world, 0.1);
        }
        assert!(!world.exists(entity));
    }
}
