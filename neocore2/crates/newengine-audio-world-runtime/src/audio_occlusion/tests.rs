use super::*;

fn listener_world() -> World {
    let mut world = World::new();
    world.insert_resource(AudioListenerRuntimeState::default());
    world
}

#[test]
fn provider_emits_bounded_multi_ray_batch_for_nearest_spatial_emitters() {
    let mut world = listener_world();
    for index in 0..3 {
        let entity = world.spawn();
        let _ = world.insert(
            entity,
            Transform {
                position: Vec3::new(0.0, 0.0, -(5.0 + index as f32 * 5.0)),
                ..Transform::default()
            },
        );
        let mut emitter = AudioEmitter::new("shared/audio/test.yscd@test");
        emitter.occlusion.ray_count = 3;
        let _ = world.insert(entity, emitter);
    }
    let provider = AudioOcclusionPhysicsQueryProvider::with_emitter_budget(2);
    let queries = provider.collect_queries(&world);
    assert_eq!(queries.len(), 8);
    assert!(queries
        .iter()
        .all(|query| query.seq & 0xfff0_0000_0000_0000 == AUDIO_OCCLUSION_QUERY_NAMESPACE));
}

#[test]
fn crowded_scene_budget_keeps_nearest_emitter_and_rotates_fair_slots() {
    let mut world = listener_world();
    let mut emitter_keys = Vec::new();
    for index in 0..6 {
        let entity = world.spawn();
        emitter_keys.push(entity.stable_u64());
        let _ = world.insert(
            entity,
            Transform {
                position: Vec3::new(0.0, 0.0, -(2.0 + index as f32 * 2.0)),
                ..Transform::default()
            },
        );
        let mut emitter = AudioEmitter::new("shared/audio/test.yscd@test");
        emitter.occlusion.ray_count = 1;
        let _ = world.insert(entity, emitter);
    }

    let provider = AudioOcclusionPhysicsQueryProvider::with_emitter_budget(2);
    let nearest = emitter_keys[0];
    let mut sampled = BTreeSet::new();
    for _ in 0..6 {
        let _ = provider.collect_queries(&world);
        let selected = provider
            .pending
            .lock()
            .values()
            .map(|probe| probe.emitter_key)
            .collect::<BTreeSet<_>>();
        assert!(selected.contains(&nearest));
        assert_eq!(selected.len(), 2);
        sampled.extend(selected);
    }
    assert_eq!(sampled.len(), emitter_keys.len());
}

#[test]
fn partial_probe_blockage_is_obstruction_not_full_occlusion() {
    let mut world = listener_world();
    let emitter_entity = world.spawn();
    let _ = world.insert(
        emitter_entity,
        Transform {
            position: Vec3::new(0.0, 0.0, -10.0),
            ..Transform::default()
        },
    );
    let mut emitter = AudioEmitter::new("shared/audio/test.yscd@test");
    emitter.occlusion.ray_count = 3;
    let _ = world.insert(emitter_entity, emitter);
    let blocker = world.spawn();

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    assert_eq!(queries.len(), 4);
    // sample 0 forward, sample 0 reverse, sample 1 forward, sample 2 forward
    let hit = PhysicsQueryHitDto {
        subshape_id: 0,
        hit_index: 0,
        back_face: false,

        seq: queries[2].seq,
        entity: blocker.stable_u64(),
        position: [0.0, 0.0, -4.0],
        normal: [0.0, 0.0, 1.0],
        distance: 4.0,
    };
    let keys = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    let consumed = provider.resolve_query_hits(&mut world, 7, &[hit], &keys);
    assert_eq!(consumed.len(), 4);
    let observation = world
        .get::<AudioOcclusionObservation>(emitter_entity)
        .cloned()
        .expect("observation");
    assert_eq!(observation.samples, 3);
    assert_eq!(observation.blocked_samples, 1);
    assert!((observation.obstruction - 1.0 / 3.0).abs() < 1.0e-6);
    assert!(observation.occlusion > 0.0 && observation.occlusion < 0.2);
}

#[test]
fn all_probe_rays_blocked_produces_full_occlusion() {
    let mut world = listener_world();
    let emitter_entity = world.spawn();
    let _ = world.insert(
        emitter_entity,
        Transform {
            position: Vec3::new(0.0, 0.0, -10.0),
            ..Transform::default()
        },
    );
    let mut emitter = AudioEmitter::new("shared/audio/test.yscd@test");
    emitter.occlusion.ray_count = 3;
    let _ = world.insert(emitter_entity, emitter);
    let blocker = world.spawn();

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    let hits = queries
        .iter()
        .map(|query| PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: query.seq,
            entity: blocker.stable_u64(),
            position: [0.0, 0.0, -4.0],
            normal: [0.0, 0.0, 1.0],
            distance: 4.0,
        })
        .collect::<Vec<_>>();
    let keys = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    provider.resolve_query_hits(&mut world, 9, &hits, &keys);
    let observation = world
        .get::<AudioOcclusionObservation>(emitter_entity)
        .cloned()
        .expect("observation");
    assert_eq!(observation.blocked_samples, 3);
    assert_eq!(observation.obstruction, 1.0);
    assert_eq!(observation.occlusion, 1.0);
}
#[test]
fn bidirectional_center_probe_resolves_single_blocker_thickness() {
    let mut world = listener_world();
    let emitter_entity = world.spawn();
    let _ = world.insert(
        emitter_entity,
        Transform {
            position: Vec3::new(0.0, 0.0, -10.0),
            ..Transform::default()
        },
    );
    let mut emitter = AudioEmitter::new("shared/audio/test.yscd@test");
    emitter.occlusion.ray_count = 1;
    let _ = world.insert(emitter_entity, emitter);
    let blocker = world.spawn();
    let _ = world.insert(
        blocker,
        AcousticSurface::new(
            "material.test.wall",
            AcousticMaterialProfile {
                transmission_gain: 0.40,
                reflection_gain: 0.55,
                high_frequency_absorption: 0.60,
                low_pass_hz: 4_000.0,
            },
        ),
    );

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    assert_eq!(queries.len(), 2);
    let hits = [
        PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: queries[0].seq,
            entity: blocker.stable_u64(),
            position: [0.0, 0.0, -4.0],
            normal: [0.0, 0.0, 1.0],
            distance: 4.0,
        },
        PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: queries[1].seq,
            entity: blocker.stable_u64(),
            position: [0.0, 0.0, -4.4],
            normal: [0.0, 0.0, -1.0],
            distance: 5.6,
        },
    ];
    let keys = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    provider.resolve_query_hits(&mut world, 11, &hits, &keys);
    let observation = world
        .get::<AudioOcclusionObservation>(emitter_entity)
        .expect("observation");
    assert!((observation.estimated_thickness_m - 0.4).abs() < 1.0e-4);
    assert_eq!(observation.center_blocker_layers, 1);
    assert!(observation.material.transmission_gain < 0.40);
    assert!(observation.material.low_pass_hz < 4_000.0);
}

#[test]
fn thick_geometry_transmits_less_energy_and_high_frequency_than_thin_geometry() {
    let base = AcousticMaterialProfile {
        transmission_gain: 0.40,
        reflection_gain: 0.55,
        high_frequency_absorption: 0.60,
        low_pass_hz: 4_000.0,
    };
    let thin = material_response_for_thickness(base, 0.05);
    let reference = material_response_for_thickness(base, ACOUSTIC_REFERENCE_THICKNESS_M);
    let thick = material_response_for_thickness(base, 0.75);
    assert!(thin.transmission_gain > reference.transmission_gain);
    assert!(reference.transmission_gain > thick.transmission_gain);
    assert!(thin.high_frequency_absorption < reference.high_frequency_absorption);
    assert!(reference.high_frequency_absorption < thick.high_frequency_absorption);
    assert!(thin.low_pass_hz > reference.low_pass_hz);
    assert!(reference.low_pass_hz > thick.low_pass_hz);
}

#[test]
fn distinct_center_blockers_accumulate_as_multiple_material_layers() {
    let a = AcousticMaterialProfile {
        transmission_gain: 0.5,
        reflection_gain: 0.45,
        high_frequency_absorption: 0.5,
        low_pass_hz: 5_000.0,
    };
    let b = AcousticMaterialProfile {
        transmission_gain: 0.4,
        reflection_gain: 0.35,
        high_frequency_absorption: 0.7,
        low_pass_hz: 3_000.0,
    };
    let combined = combine_material_layers(a, b);
    assert!((combined.transmission_gain - 0.2).abs() < 1.0e-6);
    assert!(combined.high_frequency_absorption > a.high_frequency_absorption);
    assert!(combined.high_frequency_absorption > b.high_frequency_absorption);
    assert_eq!(combined.low_pass_hz, 3_000.0);

    let aggregate = ProbeAggregate {
        center_forward: Some(ProbeBlocker {
            entity_key: 10,
            distance: 2.0,
            max_t: 10.0,
            material_id: "a".to_owned(),
            material: a,
        }),
        center_reverse: Some(ProbeBlocker {
            entity_key: 20,
            distance: 3.0,
            max_t: 10.0,
            material_id: "b".to_owned(),
            material: b,
        }),
        ..ProbeAggregate::default()
    };
    assert_eq!(center_path_geometry(&aggregate), (0.0, 2));
}

#[test]
fn center_blockage_has_more_occlusion_weight_than_peripheral_blockage() {
    let peripheral = occlusion_from_probe_coverage(1.0 / 3.0, false);
    let center = occlusion_from_probe_coverage(1.0 / 3.0, true);
    assert!(center > peripheral * 4.0);
    assert!(center < 1.0);
}

#[test]
fn authored_material_library_resolves_physics_surface_without_engine_presets() {
    let mut world = listener_world();
    world.insert_resource(AcousticMaterialLibrary::new(vec![
        newengine_audio_api::AcousticMaterialRule {
            material_id: "material.test.solid".to_owned(),
            surface_matches: vec!["test_solid".to_owned()],
            profile: AcousticMaterialProfile {
                transmission_gain: 0.21,
                reflection_gain: 0.67,
                high_frequency_absorption: 0.81,
                low_pass_hz: 2_100.0,
            },
        },
    ]));
    let blocker = world.spawn();
    let _ = world.insert(
        blocker,
        PhysicsSurface {
            id: "surface.wall.test_solid".to_owned(),
            ..PhysicsSurface::default()
        },
    );
    let keys = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    let hit = PhysicsQueryHitDto {
        subshape_id: 0,
        hit_index: 0,
        back_face: false,

        seq: 1,
        entity: blocker.stable_u64(),
        position: [0.0; 3],
        normal: [0.0, 1.0, 0.0],
        distance: 1.0,
    };
    let resolved = resolve_probe_blocker(&world, &keys, &hit, 4.0);
    assert_eq!(resolved.material_id, "material.test.solid");
    assert!((resolved.material.transmission_gain - 0.21).abs() < 1.0e-6);
}

#[test]
fn unmapped_physics_surface_uses_transparent_material_fallback() {
    let mut world = listener_world();
    let blocker = world.spawn();
    let _ = world.insert(
        blocker,
        PhysicsSurface {
            id: "surface.project.unknown".to_owned(),
            ..PhysicsSurface::default()
        },
    );
    let keys = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    let hit = PhysicsQueryHitDto {
        subshape_id: 0,
        hit_index: 0,
        back_face: false,

        seq: 1,
        entity: blocker.stable_u64(),
        position: [0.0; 3],
        normal: [0.0, 1.0, 0.0],
        distance: 1.0,
    };
    let resolved = resolve_probe_blocker(&world, &keys, &hit, 4.0);
    assert_eq!(resolved.material_id, "surface.project.unknown");
    assert_eq!(resolved.material, AcousticMaterialProfile::transparent());
}
