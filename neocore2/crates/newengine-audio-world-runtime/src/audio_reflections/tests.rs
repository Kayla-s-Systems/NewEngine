use super::*;
use newengine_audio_api::{AcousticMaterialProfile, AcousticSurface, AudioListenerState};

fn reflection_world() -> (World, EntityId) {
    let mut world = World::new();
    world.insert_resource(AudioListenerRuntimeState {
        listener: AudioListenerState::default(),
        listener_entity: None,
        frame_index: 1,
    });

    let room = world.spawn();
    let _ = world.insert(room, Transform::default());
    let _ = world.insert(
        room,
        AudioEnvironmentZone {
            zone_id: "room.reflection-test".to_owned(),
            half_extents: [5.0, 4.0, 6.0],
            blend_distance: 0.0,
            ..AudioEnvironmentZone::default()
        },
    );

    let emitter = world.spawn();
    let _ = world.insert(
        emitter,
        Transform {
            position: Vec3::new(1.0, 0.5, -1.0),
            ..Transform::default()
        },
    );
    let _ = world.insert(
        emitter,
        AudioEmitter::new("shared/audio/test.ysncd@reflection"),
    );
    (world, emitter)
}

fn entity_keys(world: &World) -> BTreeMap<u64, EntityId> {
    world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect()
}

fn ray_max_t(query: &PhysicsQueryDto) -> f32 {
    match query.kind {
        PhysicsQueryKindDto::Ray { max_t, .. } => max_t,
        _ => panic!("reflection contributor must emit ray queries"),
    }
}

#[test]
fn provider_emits_two_visibility_legs_for_each_first_order_room_face() {
    let (world, _) = reflection_world();
    let provider = AudioReflectionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    assert_eq!(provider.pending.lock().len(), 12);
    assert_eq!(provider.pending_second_order.lock().len(), 12);
    assert_eq!(queries.len(), 24);
    assert!(queries
        .iter()
        .all(|query| { query.seq & 0xfff0_0000_0000_0000 == AUDIO_REFLECTION_QUERY_NAMESPACE }));
}

#[test]
fn reflection_queries_sample_at_secondary_acoustic_cadence() {
    let (world, _) = reflection_world();
    let provider = AudioReflectionPhysicsQueryProvider::new();
    assert!(
        !provider.collect_queries(&world).is_empty(),
        "first acoustic sample is due"
    );
    for skipped in 1..REFLECTION_QUERY_INTERVAL_TICKS {
        assert!(
            provider.collect_queries(&world).is_empty(),
            "reflection query batch must be skipped at cadence offset {skipped}"
        );
    }
    assert!(
        !provider.collect_queries(&world).is_empty(),
        "reflection query batch must resume at the next cadence boundary"
    );
}

#[test]
fn clear_reflection_probes_publish_visible_material_unknown_paths() {
    let (mut world, emitter) = reflection_world();
    let provider = AudioReflectionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    let keys = entity_keys(&world);
    let consumed = provider.resolve_query_hits(&mut world, 7, &[], &keys);
    assert_eq!(consumed.len(), queries.len());
    let observation = world
        .get::<AudioEarlyReflectionObservation>(emitter)
        .expect("reflection observation");
    assert_eq!(observation.paths.len(), 6);
    assert_eq!(observation.second_order_paths.len(), 4);
    assert!(observation.paths.iter().all(|path| path.visible));
    assert!(observation.paths.iter().all(|path| !path.material_known));
    assert!(observation
        .second_order_paths
        .iter()
        .all(|path| path.visible));
    assert!(observation
        .second_order_paths
        .iter()
        .all(|path| path.material_known == [false; 2]));
}

#[test]
fn blocker_before_reflection_point_closes_only_that_first_order_path() {
    let (mut world, emitter) = reflection_world();
    let blocker = world.spawn();
    let provider = AudioReflectionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    let query = &queries[0];
    let max_t = ray_max_t(query);
    assert!(max_t > 0.5);
    let hit = PhysicsQueryHitDto {
        subshape_id: 0,
        hit_index: 0,
        back_face: false,

        seq: query.seq,
        entity: blocker.stable_u64(),
        position: [0.0; 3],
        normal: [0.0, 1.0, 0.0],
        distance: max_t * 0.5,
    };
    let keys = entity_keys(&world);
    provider.resolve_query_hits(&mut world, 8, &[hit], &keys);
    let observation = world
        .get::<AudioEarlyReflectionObservation>(emitter)
        .expect("reflection observation");
    assert_eq!(
        observation
            .paths
            .iter()
            .filter(|path| !path.visible)
            .count(),
        1
    );
    assert_eq!(
        observation.paths.iter().filter(|path| path.visible).count(),
        5
    );
}

#[test]
fn endpoint_hit_resolves_authored_boundary_reflection_material() {
    let (mut world, emitter) = reflection_world();
    let boundary = world.spawn();
    let authored = AcousticMaterialProfile {
        transmission_gain: 0.18,
        reflection_gain: 0.91,
        high_frequency_absorption: 0.24,
        low_pass_hz: 7_500.0,
    };
    let _ = world.insert(
        boundary,
        AcousticSurface::new("material.test.reflective", authored),
    );

    let provider = AudioReflectionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    let query = &queries[0];
    let max_t = ray_max_t(query);
    let hit = PhysicsQueryHitDto {
        subshape_id: 0,
        hit_index: 0,
        back_face: false,

        seq: query.seq,
        entity: boundary.stable_u64(),
        position: [0.0; 3],
        normal: [0.0, 1.0, 0.0],
        distance: max_t,
    };
    let keys = entity_keys(&world);
    provider.resolve_query_hits(&mut world, 9, &[hit], &keys);
    let observation = world
        .get::<AudioEarlyReflectionObservation>(emitter)
        .expect("reflection observation");
    let resolved = observation
        .paths
        .iter()
        .find(|path| path.material_known)
        .expect("authored endpoint material");
    assert!(resolved.visible);
    assert_eq!(resolved.boundary_entity, Some(boundary.stable_u64()));
    assert!((resolved.material.reflection_gain - 0.91).abs() < 1.0e-6);
    assert!((resolved.material.high_frequency_absorption - 0.24).abs() < 1.0e-6);
}

#[test]
fn second_order_middle_blocker_closes_only_its_three_leg_path() {
    let (mut world, emitter) = reflection_world();
    let blocker = world.spawn();
    let provider = AudioReflectionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    let (seq, pending) = provider
        .pending_second_order
        .lock()
        .iter()
        .find(|(_, pending)| pending.leg == SecondOrderProbeLeg::Middle)
        .map(|(seq, pending)| (*seq, *pending))
        .expect("second-order middle leg");
    let query = queries
        .iter()
        .find(|query| query.seq == seq)
        .expect("middle query");
    let max_t = ray_max_t(query);
    let hit = PhysicsQueryHitDto {
        subshape_id: 0,
        hit_index: 0,
        back_face: false,
        seq,
        entity: blocker.stable_u64(),
        position: [0.0; 3],
        normal: [0.0, 1.0, 0.0],
        distance: max_t * 0.5,
    };
    let keys = entity_keys(&world);
    provider.resolve_query_hits(&mut world, 10, &[hit], &keys);
    let observation = world
        .get::<AudioEarlyReflectionObservation>(emitter)
        .expect("reflection observation");
    let blocked = observation
        .second_order_paths
        .iter()
        .filter(|path| !path.visible)
        .collect::<Vec<_>>();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].face_indices, pending.geometry.face_indices);
    assert!(observation.paths.iter().all(|path| path.visible));
}

#[test]
fn second_order_endpoint_hits_resolve_two_independent_boundary_materials() {
    let (mut world, emitter) = reflection_world();
    let first_boundary = world.spawn();
    let second_boundary = world.spawn();
    let first_material = AcousticMaterialProfile {
        transmission_gain: 0.2,
        reflection_gain: 0.82,
        high_frequency_absorption: 0.25,
        low_pass_hz: 7_000.0,
    };
    let second_material = AcousticMaterialProfile {
        transmission_gain: 0.3,
        reflection_gain: 0.55,
        high_frequency_absorption: 0.60,
        low_pass_hz: 3_500.0,
    };
    let _ = world.insert(
        first_boundary,
        AcousticSurface::new("material.test.first", first_material),
    );
    let _ = world.insert(
        second_boundary,
        AcousticSurface::new("material.test.second", second_material),
    );

    let provider = AudioReflectionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    let pending = provider.pending_second_order.lock();
    let target_faces = pending
        .values()
        .find(|pending| pending.leg == SecondOrderProbeLeg::Source)
        .map(|pending| pending.geometry.face_indices)
        .expect("second-order path");
    let source = pending
        .iter()
        .find(|(_, pending)| {
            pending.geometry.face_indices == target_faces
                && pending.leg == SecondOrderProbeLeg::Source
        })
        .map(|(seq, pending)| (*seq, *pending))
        .expect("source endpoint leg");
    let listener = pending
        .iter()
        .find(|(_, pending)| {
            pending.geometry.face_indices == target_faces
                && pending.leg == SecondOrderProbeLeg::Listener
        })
        .map(|(seq, pending)| (*seq, *pending))
        .expect("listener endpoint leg");
    drop(pending);
    let source_max = ray_max_t(
        queries
            .iter()
            .find(|query| query.seq == source.0)
            .expect("source query"),
    );
    let listener_max = ray_max_t(
        queries
            .iter()
            .find(|query| query.seq == listener.0)
            .expect("listener query"),
    );
    let hits = [
        PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: source.0,
            entity: first_boundary.stable_u64(),
            position: source.1.geometry.reflection_points[0],
            normal: [0.0, 1.0, 0.0],
            distance: source_max,
        },
        PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: listener.0,
            entity: second_boundary.stable_u64(),
            position: listener.1.geometry.reflection_points[1],
            normal: [0.0, 1.0, 0.0],
            distance: listener_max,
        },
    ];
    let keys = entity_keys(&world);
    provider.resolve_query_hits(&mut world, 11, &hits, &keys);
    let observation = world
        .get::<AudioEarlyReflectionObservation>(emitter)
        .expect("reflection observation");
    let path = observation
        .second_order_paths
        .iter()
        .find(|path| path.face_indices == target_faces)
        .expect("resolved second-order path");
    assert!(path.visible);
    assert_eq!(
        path.boundary_entities,
        [
            Some(first_boundary.stable_u64()),
            Some(second_boundary.stable_u64()),
        ]
    );
    assert_eq!(path.material_known, [true, true]);
    assert_eq!(path.materials, [first_material, second_material]);
}
