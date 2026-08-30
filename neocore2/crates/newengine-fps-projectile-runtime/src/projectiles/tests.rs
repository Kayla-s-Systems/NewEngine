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

    fn install_test_weapon_vfx(world: &mut World) {
        use newengine_vfx_api::{VfxEffectRef, VfxGpuBillboardMode, VfxPriority};
        use newengine_vfx_runtime::{
            VfxAlignment, VfxEffectDefinition, VfxEffectLibrary, VfxLayerDefinition, VfxLayerKind,
            VfxRenderRole,
        };
        let mut library = VfxEffectLibrary::default();
        library
            .register(VfxEffectDefinition {
                effect: VfxEffectRef::new("effects/test_weapon.fxd@shot"),
                priority: VfxPriority::High,
                layers: vec![
                    VfxLayerDefinition::Pulse {
                        kind: VfxLayerKind::MuzzleFlash,
                        primitive: prim_builtins::ID_PLANE,
                        role: VfxRenderRole::Transparent,
                        alignment: VfxAlignment::DirectionZ,
                        texture_slot: 0,
                        billboard: VfxGpuBillboardMode::CameraFacing,
                        offset_along_direction: 0.0,
                        offset_along_normal: 0.0,
                        scale: Vec3::splat(0.1),
                        growth_per_second: Vec3::ZERO,
                        color: [1.0; 4],
                        lifetime_seconds: 0.05,
                        fade_start_fraction: 0.5, fade_in_fraction: 0.0, drag_per_second: 0.0, rotation_radians: 0.0, rotation_random_radians: 0.0, spin_radians_per_second: 0.0,
                        light: None,
                    },
                    VfxLayerDefinition::Pulse {
                        kind: VfxLayerKind::MuzzleCore,
                        primitive: prim_builtins::ID_SPHERE_UV,
                        role: VfxRenderRole::Transparent,
                        alignment: VfxAlignment::None,
                        texture_slot: 0,
                        billboard: VfxGpuBillboardMode::CameraFacing,
                        offset_along_direction: 0.0,
                        offset_along_normal: 0.0,
                        scale: Vec3::splat(0.05),
                        growth_per_second: Vec3::ZERO,
                        color: [1.0; 4],
                        lifetime_seconds: 0.04,
                        fade_start_fraction: 0.5, fade_in_fraction: 0.0, drag_per_second: 0.0, rotation_radians: 0.0, rotation_random_radians: 0.0, spin_radians_per_second: 0.0,
                        light: None,
                    },
                    VfxLayerDefinition::Pulse {
                        kind: VfxLayerKind::Smoke,
                        primitive: prim_builtins::ID_SPHERE_UV,
                        role: VfxRenderRole::Transparent,
                        alignment: VfxAlignment::DirectionZ,
                        texture_slot: 0,
                        billboard: VfxGpuBillboardMode::CameraFacing,
                        offset_along_direction: 0.0,
                        offset_along_normal: 0.0,
                        scale: Vec3::splat(0.05),
                        growth_per_second: Vec3::splat(0.1),
                        color: [0.2, 0.2, 0.2, 0.3],
                        lifetime_seconds: 0.5,
                        fade_start_fraction: 0.5, fade_in_fraction: 0.0, drag_per_second: 0.0, rotation_radians: 0.0, rotation_random_radians: 0.0, spin_radians_per_second: 0.0,
                        light: None,
                    },
                    VfxLayerDefinition::Tracer {
                        primitive: prim_builtins::ID_CUBE,
                        color: [1.0, 0.7, 0.2, 1.0],
                        half_length: 0.18,
                        radius: 0.003,
                        speed: 180.0,
                        max_lifetime_seconds: 0.65,
                    },
                ],
            })
            .expect("register shot test effect");
        library
            .register(VfxEffectDefinition {
                effect: VfxEffectRef::new("effects/test_weapon.fxd@impact.metal"),
                priority: VfxPriority::High,
                layers: vec![
                    VfxLayerDefinition::Burst {
                        kind: VfxLayerKind::Spark,
                        primitive: prim_builtins::ID_CUBE,
                        role: VfxRenderRole::Transparent,
                        texture_slot: 0,
                        billboard: VfxGpuBillboardMode::VelocityAligned,
                        count: 8,
                        scale: Vec3::splat(0.01),
                        color: [1.0, 0.7, 0.2, 1.0],
                        speed_min: 2.0,
                        speed_max: 7.0, cone_angle_degrees: 70.0, size_variance: 0.2, lifetime_variance: 0.15, drag_per_second: 0.1, rotation_random_radians: 3.14159, spin_radians_per_second: 3.0, spin_variance: 1.5,
                        acceleration: Vec3::new(0.0, -9.8, 0.0),
                        lifetime_seconds: 0.3,
                        fade_start_fraction: 0.5, fade_in_fraction: 0.0,
                    },
                    VfxLayerDefinition::Decal {
                        primitive: prim_builtins::ID_DISC,
                        scale: Vec3::new(0.1, 0.002, 0.1),
                        color: [0.05, 0.05, 0.05, 1.0],
                        normal_offset: 0.003,
                        lifetime_seconds: 5.0,
                        fade_start_fraction: 0.9,
                    },
                ],
            })
            .expect("register impact test effect");
        world.insert_resource(library);
    }

    #[test]
    fn weapon_shot_fx_starts_at_muzzle_and_stops_at_hitscan_impact() {
        let mut world = World::new();
        let mut authored = newengine_item_assets_runtime::test_fps_item_package();
        let rifle = authored
            .items
            .iter_mut()
            .find(|item| item.id == "weapon.rifle.standard")
            .expect("authored test rifle");
        rifle.weapon_vfx = Some(newengine_item_assets_runtime::AuthoredWeaponVfxDefinition {
            shot: "effects/test_weapon.fxd@shot".to_owned(),
            impact_default: "effects/test_weapon.fxd@impact.metal".to_owned(),
            impact_by_surface: [(
                "surface.metal".to_owned(),
                "effects/test_weapon.fxd@impact.metal".to_owned(),
            )]
            .into_iter()
            .collect(),
        });
        let package = newengine_item_assets_runtime::compile_authored_item_package(&authored)
            .expect("compile test item package");
        newengine_item_assets_runtime::install_compiled_item_package(&mut world, package);
        install_test_weapon_vfx(&mut world);
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
