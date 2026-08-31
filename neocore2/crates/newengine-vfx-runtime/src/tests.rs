use newengine_ecs::World;
use newengine_math::Vec3;
use newengine_model_domain_api::{MeshRenderOptions, MeshRenderRole};
use newengine_vfx_api::{
    EntityHandle, VfxBudgetV1, VfxEffectRef, VfxGpuBillboardMode, VfxPriority, VfxSpawnRequestV1,
};

use crate::*;

const TEST_SHOT_EFFECT: &str = "effects/tests/weapon.fxd@shot";
const TEST_IMPACT_EFFECT: &str = "effects/tests/weapon.fxd@impact";

fn test_effect_library() -> VfxEffectLibrary {
    let mut library = VfxEffectLibrary::default();
    library.register(VfxEffectDefinition {
        effect: VfxEffectRef::new(TEST_SHOT_EFFECT),
        priority: VfxPriority::High,
        layers: vec![
            VfxLayerDefinition::Pulse {
                kind: VfxLayerKind::MuzzleFlash, primitive: newengine_primitives::builtins::ID_PLANE,
                role: VfxRenderRole::Transparent, alignment: VfxAlignment::DirectionZ, texture_slot: 0,
                billboard: VfxGpuBillboardMode::CameraFacing, offset_along_direction: 0.0, offset_along_normal: 0.0,
                scale: Vec3::splat(0.1), growth_per_second: Vec3::ZERO, color: [1.0;4], lifetime_seconds: 0.05,
                fade_start_fraction: 0.5, fade_in_fraction: 0.0, drag_per_second: 0.0, rotation_radians: 0.0, rotation_random_radians: 0.0, spin_radians_per_second: 0.0, light: Some(VfxLightDefinition { color: [1.0,0.7,0.3], intensity: 8.0, range: 2.0 }),
            },
            VfxLayerDefinition::Pulse {
                kind: VfxLayerKind::MuzzleCore, primitive: newengine_primitives::builtins::ID_SPHERE_UV,
                role: VfxRenderRole::Transparent, alignment: VfxAlignment::None, texture_slot: 0,
                billboard: VfxGpuBillboardMode::CameraFacing, offset_along_direction: 0.0, offset_along_normal: 0.0,
                scale: Vec3::splat(0.04), growth_per_second: Vec3::ZERO, color: [1.0;4], lifetime_seconds: 0.04,
                fade_start_fraction: 0.5, fade_in_fraction: 0.0, drag_per_second: 0.0, rotation_radians: 0.0, rotation_random_radians: 0.0, spin_radians_per_second: 0.0, light: None,
            },
            VfxLayerDefinition::Pulse {
                kind: VfxLayerKind::Smoke, primitive: newengine_primitives::builtins::ID_SPHERE_UV,
                role: VfxRenderRole::Transparent, alignment: VfxAlignment::DirectionZ, texture_slot: 0,
                billboard: VfxGpuBillboardMode::CameraFacing, offset_along_direction: 0.0, offset_along_normal: 0.0,
                scale: Vec3::splat(0.05), growth_per_second: Vec3::splat(0.1), color: [0.2,0.2,0.2,0.3],
                lifetime_seconds: 0.5, fade_start_fraction: 0.5, fade_in_fraction: 0.0, drag_per_second: 0.0, rotation_radians: 0.0, rotation_random_radians: 0.0, spin_radians_per_second: 0.0, light: None,
            },
            VfxLayerDefinition::Tracer {
                primitive: newengine_primitives::builtins::ID_CUBE, color: [1.0,0.7,0.2,1.0],
                half_length: 0.18, radius: 0.003, speed: 12.0, max_lifetime_seconds: 1.0,
            },
        ],
    }).unwrap();
    library.register(VfxEffectDefinition {
        effect: VfxEffectRef::new(TEST_IMPACT_EFFECT),
        priority: VfxPriority::High,
        layers: vec![
            VfxLayerDefinition::Burst {
                kind: VfxLayerKind::Spark, primitive: newengine_primitives::builtins::ID_CUBE,
                role: VfxRenderRole::Transparent, texture_slot: 0, billboard: VfxGpuBillboardMode::VelocityAligned,
                count: 8, scale: Vec3::splat(0.01), color: [1.0,0.82,0.25,1.0], speed_min: 2.0, speed_max: 7.0, cone_angle_degrees: 70.0, size_variance: 0.2, lifetime_variance: 0.15, drag_per_second: 0.1, rotation_random_radians: 3.14159, spin_radians_per_second: 3.0, spin_variance: 1.5,
                acceleration: Vec3::new(0.0,-9.8,0.0), lifetime_seconds: 0.3, fade_start_fraction: 0.5, fade_in_fraction: 0.0,
            },
            VfxLayerDefinition::Pulse {
                kind: VfxLayerKind::Smoke, primitive: newengine_primitives::builtins::ID_SPHERE_UV,
                role: VfxRenderRole::Transparent, alignment: VfxAlignment::NormalY, texture_slot: 0,
                billboard: VfxGpuBillboardMode::CameraFacing, offset_along_direction: 0.0, offset_along_normal: 0.01,
                scale: Vec3::splat(0.04), growth_per_second: Vec3::splat(0.1), color: [0.2,0.2,0.2,0.3],
                lifetime_seconds: 0.5, fade_start_fraction: 0.5, fade_in_fraction: 0.0, drag_per_second: 0.0, rotation_radians: 0.0, rotation_random_radians: 0.0, spin_radians_per_second: 0.0, light: None,
            },
            VfxLayerDefinition::Decal {
                primitive: newengine_primitives::builtins::ID_DISC, scale: Vec3::new(0.1,0.002,0.1),
                color: [0.05,0.05,0.05,1.0], normal_offset: 0.003, lifetime_seconds: 5.0, fade_start_fraction: 0.9,
            },
        ],
    }).unwrap();
    library
}

fn test_world() -> World {
    let mut world = World::new();
    world.insert_resource(test_effect_library());
    world
}

fn shot_request(owner: u64, sequence: u64) -> VfxSpawnRequestV1 {
    VfxSpawnRequestV1 {
        effect: VfxEffectRef::new(TEST_SHOT_EFFECT),
        owner: Some(EntityHandle::new(owner)),
        correlation_id: sequence,
        position: [1.0, 2.0, 3.0],
        direction: [0.0, 0.0, -1.0],
        max_distance: 100.0,
        seed: sequence,
        ..Default::default()
    }
}

#[test]
fn default_library_contains_no_weapon_presets() {
    assert!(VfxEffectLibrary::default().is_empty());
    let library = test_effect_library();
    assert_eq!(library.get(TEST_SHOT_EFFECT).unwrap().layers.len(), 4);
    assert_eq!(library.get(TEST_IMPACT_EFFECT).unwrap().estimated_layers(), 10);
}

#[test]
fn weapon_shot_routes_muzzle_and_smoke_to_gpu_without_near_camera_primitives() {
    let mut world = test_world();
    let id = spawn_vfx(&mut world, shot_request(9, 17))
        .unwrap()
        .expect("spawned VFX");
    assert_eq!(world.query::<VfxInstanceRoot>().count(), 1);
    let layers = world
        .query::<VfxLayerRuntime>()
        .map(|(entity, layer)| (entity, *layer))
        .collect::<Vec<_>>();
    assert_eq!(
        layers.len(),
        1,
        "muzzle flash/core and smoke must stay out of ECS primitive rendering"
    );
    assert!(layers.iter().all(|(_, layer)| layer.instance_id == id));
    assert_eq!(
        layers
            .iter()
            .filter(|(_, layer)| layer.kind == VfxLayerKind::Tracer)
            .count(),
        1
    );
    assert_eq!(
        world.query::<newengine_lighting::PointLight>().count(),
        1,
        "GPU muzzle core must retain its authored transient point light"
    );
    assert!(layers.iter().all(|(entity, _)| {
        world
            .get::<MeshRenderOptions>(*entity)
            .is_some_and(|options| options.role == MeshRenderRole::WorldTransparent)
    }));
    let ledger = world
        .resource::<VfxGpuParticleLedger>()
        .expect("GPU particle ledger");
    assert_eq!(ledger.layers().len(), 3);
    assert!(ledger
        .layers()
        .iter()
        .any(|layer| layer.kind == VfxLayerKind::MuzzleFlash && layer.particle_count == 1));
    assert!(ledger
        .layers()
        .iter()
        .any(|layer| layer.kind == VfxLayerKind::MuzzleCore && layer.particle_count == 1));
    assert!(ledger
        .layers()
        .iter()
        .any(|layer| layer.kind == VfxLayerKind::Smoke && layer.particle_count == 1));
    let bridge = world
        .resource::<newengine_vfx_api::VfxGpuParticleBridge>()
        .unwrap();
    let spawns = bridge.drain_spawns(8);
    assert_eq!(spawns.len(), 3);
    assert!(spawns.iter().all(|spawn| spawn.instance_id == id.0));
    assert!(spawns
        .iter()
        .any(|spawn| spawn.kind == newengine_vfx_api::VfxGpuParticleKind::MuzzleFlash));
    assert!(spawns
        .iter()
        .any(|spawn| spawn.kind == newengine_vfx_api::VfxGpuParticleKind::MuzzleCore));
    assert!(spawns
        .iter()
        .any(|spawn| spawn.kind == newengine_vfx_api::VfxGpuParticleKind::Smoke));
    assert_eq!(vfx_runtime_stats(&world).active_layers, 4);
}

#[test]
fn tracer_clamp_stops_at_authoritative_hit() {
    let mut world = test_world();
    spawn_vfx(&mut world, shot_request(11, 3)).unwrap();
    let point = Vec3::new(1.0, 2.0, 2.0);
    clamp_vfx_tracers_to_hit(&mut world, 11, 3, point);
    let tracer = world
        .query::<VfxLayerRuntime>()
        .find_map(|(_, layer)| (layer.kind == VfxLayerKind::Tracer).then_some(*layer))
        .expect("tracer");
    assert!((tracer.max_distance - 1.0).abs() < 1.0e-5);
    for _ in 0..8 {
        step_vfx(&mut world, 0.01);
    }
    assert_eq!(
        world
            .query::<VfxLayerRuntime>()
            .filter(|(_, layer)| layer.kind == VfxLayerKind::Tracer)
            .count(),
        0
    );
}

#[test]
fn impact_routes_surface_aware_sparks_and_smoke_to_gpu_and_keeps_decal_in_ecs() {
    let mut world = test_world();
    let request = VfxSpawnRequestV1 {
        effect: VfxEffectRef::new(TEST_IMPACT_EFFECT),
        position: [0.0, 1.0, 0.0],
        direction: [0.0, 0.0, -1.0],
        normal: [0.0, 1.0, 0.0],
        surface: Some("surface.metal".to_owned()),
        seed: 44,
        ..Default::default()
    };
    spawn_vfx(&mut world, request).unwrap().expect("impact");
    let decal_entity = world
        .query::<VfxLayerRuntime>()
        .find_map(|(entity, layer)| (layer.kind == VfxLayerKind::ImpactDecal).then_some(entity))
        .expect("impact decal");
    assert_eq!(
        world.get::<MeshRenderOptions>(decal_entity).unwrap().role,
        MeshRenderRole::Decal
    );
    assert_eq!(
        world
            .query::<VfxLayerRuntime>()
            .filter(|(_, layer)| matches!(layer.kind, VfxLayerKind::Spark | VfxLayerKind::Smoke))
            .count(),
        0,
        "spark/smoke particles must not allocate ECS render entities"
    );
    let bridge = world
        .resource::<newengine_vfx_api::VfxGpuParticleBridge>()
        .unwrap();
    let spawns = bridge.drain_spawns(32);
    assert_eq!(spawns.len(), 9);
    let sparks = spawns
        .iter()
        .filter(|spawn| spawn.kind == newengine_vfx_api::VfxGpuParticleKind::Spark)
        .collect::<Vec<_>>();
    assert_eq!(sparks.len(), 8);
    assert!(
        sparks.iter().all(|spark| spark.color[1] > 0.7),
        "metal impact should emit authored hot GPU sparks"
    );
    assert_eq!(
        spawns
            .iter()
            .filter(|spawn| spawn.kind == newengine_vfx_api::VfxGpuParticleKind::Smoke)
            .count(),
        1
    );
    assert_eq!(vfx_runtime_stats(&world).active_layers, 10);
}

#[test]
fn budget_is_bounded_and_reports_drops() {
    let mut world = test_world();
    world.insert_resource(VfxRuntimeState::with_budget(VfxBudgetV1 {
        max_active_instances: 1,
        max_active_layers: 4,
        max_transient_lights: 1,
        max_decals: 1,
        max_trails: 1,
        max_particle_estimate: 4,
    }));
    assert!(spawn_vfx(&mut world, shot_request(1, 1)).unwrap().is_some());
    assert!(spawn_vfx(&mut world, shot_request(1, 2)).unwrap().is_none());
    let stats = vfx_runtime_stats(&world);
    assert_eq!(stats.active_instances, 1);
    assert_eq!(stats.active_layers, 4);
    assert_eq!(stats.dropped_instances, 1);
}

#[test]
fn queued_requests_are_bounded_and_report_overflow() {
    let mut world = test_world();
    world.insert_resource(VfxSpawnQueue::with_capacity(2));
    assert!(queue_vfx(&mut world, shot_request(7, 1)).unwrap());
    assert!(queue_vfx(&mut world, shot_request(7, 2)).unwrap());
    assert!(!queue_vfx(&mut world, shot_request(7, 3)).unwrap());

    let before = vfx_runtime_stats(&world);
    assert_eq!(before.pending_requests, 2);
    assert_eq!(before.dropped_requests, 1);
    assert_eq!(before.active_instances, 0);

    let report = process_queued_vfx(&mut world);
    assert_eq!(report.processed, 2);
    assert_eq!(report.spawned, 2);
    assert_eq!(report.budget_rejected, 0);
    assert_eq!(report.failed, 0);

    let after = vfx_runtime_stats(&world);
    assert_eq!(after.pending_requests, 0);
    assert_eq!(after.dropped_requests, 1);
    assert_eq!(after.active_instances, 2);
}

#[test]
fn pending_tracer_is_clamped_before_pre_update_materialization() {
    let mut world = test_world();
    assert!(queue_vfx(&mut world, shot_request(11, 9)).unwrap());
    assert_eq!(vfx_runtime_stats(&world).pending_requests, 1);

    clamp_vfx_tracers_to_hit(&mut world, 11, 9, Vec3::new(1.0, 2.0, 2.0));
    pre_update_vfx(&mut world);

    assert_eq!(vfx_runtime_stats(&world).pending_requests, 0);
    let tracer = world
        .query::<VfxLayerRuntime>()
        .find_map(|(_, layer)| (layer.kind == VfxLayerKind::Tracer).then_some(*layer))
        .expect("queued tracer materialized during PreUpdate");
    assert!((tracer.max_distance - 1.0).abs() < 1.0e-5);
}
#[test]
fn deterministic_impact_seed_reproduces_gpu_spark_velocities() {
    fn velocities() -> Vec<[f32; 3]> {
        let mut world = test_world();
        spawn_vfx(
            &mut world,
            VfxSpawnRequestV1 {
                effect: VfxEffectRef::new(TEST_IMPACT_EFFECT),
                normal: [0.0, 1.0, 0.0],
                seed: 0x1234,
                ..Default::default()
            },
        )
        .unwrap();
        let bridge = world
            .resource::<newengine_vfx_api::VfxGpuParticleBridge>()
            .unwrap();
        bridge
            .drain_spawns(32)
            .into_iter()
            .filter(|spawn| spawn.kind == newengine_vfx_api::VfxGpuParticleKind::Spark)
            .map(|spawn| spawn.velocity)
            .collect()
    }
    let first = velocities();
    let second = velocities();
    assert_eq!(first.len(), 8);
    assert_eq!(first, second);
    assert!(first
        .iter()
        .any(|velocity| velocity.iter().any(|v| v.abs() > 0.001)));
}

#[test]
fn staged_frame_driver_exposes_reference_like_lifecycle() {
    let mut world = test_world();
    pre_update_vfx(&mut world);
    assert_eq!(
        world.resource::<VfxRuntimeState>().unwrap().stage,
        VfxRuntimeStage::PreUpdate
    );
    update_vfx(&mut world, 1.0 / 60.0);
    assert_eq!(
        world.resource::<VfxRuntimeState>().unwrap().stage,
        VfxRuntimeStage::Update
    );
    update_after_pre_render_vfx(&mut world);
    assert_eq!(
        world.resource::<VfxRuntimeState>().unwrap().stage,
        VfxRuntimeStage::AfterPreRender
    );
    finish_vfx_frame(&mut world);
    let state = world.resource::<VfxRuntimeState>().unwrap();
    assert_eq!(state.stage, VfxRuntimeStage::Idle);
    assert_eq!(state.frame_index, 1);
}
