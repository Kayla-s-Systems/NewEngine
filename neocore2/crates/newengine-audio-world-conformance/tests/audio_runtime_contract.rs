use newengine_audio_api::{
    AcousticMaterialLibrary, AcousticMaterialProfile, AcousticMaterialRule, AcousticSurface,
    AudioAmbienceBed, AudioAmbienceScope, AudioEmitter, AudioEnvironmentKind, AudioEnvironmentZone,
    AudioPortal, AudioReverbPreset,
};
use newengine_audio_client::audio_listener_from_camera_snapshot;
use newengine_audio_world_api::{
    AudioEarlyReflectionObservation, AudioEdgeDiffractionObservation, AudioListenerRuntimeState,
    AudioOcclusionObservation,
};
use newengine_audio_world_runtime::{AudioAmbienceRuntimeModule, AudioEnvironmentFrame};
use newengine_camera_contracts::CameraFrameSnapshot;
use newengine_ecs::World;
use newengine_engine_runtime::gameplay::{
    spawn_default_player, GameplayPhysicsQueryProvider, PhysicsSurface, StaticMeshCollider,
};
use newengine_engine_runtime::{
    AudioDiffractionPhysicsQueryProvider, AudioOcclusionPhysicsQueryProvider,
    AudioReflectionPhysicsQueryProvider,
};
use newengine_math::{Quat, Vec3};
use newengine_physics_api::PhysicsQueryHitDto;
use newengine_transform::Transform;
use std::collections::BTreeMap;

#[test]
fn camera_snapshot_is_the_audio_listener_contract() {
    let snapshot = CameraFrameSnapshot {
        position_ws: [12.0, 3.0, -8.0],
        forward_ws: [0.0, 0.0, -1.0],
        up_ws: [0.0, 1.0, 0.0],
        finite: true,
        ..Default::default()
    };
    let listener = audio_listener_from_camera_snapshot(&snapshot).expect("finite listener");
    assert_eq!(listener.position, snapshot.position_ws);
    assert_eq!(listener.forward, snapshot.forward_ws);
    assert_eq!(listener.up, snapshot.up_ws);
}

#[test]
fn non_finite_camera_frame_never_reaches_spatial_audio() {
    let snapshot = CameraFrameSnapshot {
        position_ws: [f32::NAN, 0.0, 0.0],
        finite: true,
        ..Default::default()
    };
    assert!(audio_listener_from_camera_snapshot(&snapshot).is_none());
}

#[test]
fn authored_audio_emitter_references_ysncd_cue_not_backend_clip() {
    let emitter = AudioEmitter::new("shared/audio/weapon/rifle/rifle.ysncd@fire");
    assert_eq!(emitter.cue, "shared/audio/weapon/rifle/rifle.ysncd@fire");
    assert!(emitter.enabled);
    assert!(emitter.autoplay);
    assert!(emitter.spatial);
}

#[test]
fn acoustic_provider_batches_multi_ray_queries_and_explicitly_ignores_listener_player() {
    let mut world = World::new();
    let player = spawn_default_player(&mut world, None, "audio-listener-player", Vec3::ZERO);
    world.insert_resource(AudioListenerRuntimeState {
        listener_entity: Some(player.stable_u64()),
        ..AudioListenerRuntimeState::default()
    });
    let emitter = world.spawn();
    let _ = world.insert(
        emitter,
        Transform {
            position: Vec3::new(0.0, 0.0, -12.0),
            ..Transform::default()
        },
    );
    let mut authored = AudioEmitter::new("shared/audio/weapon/rifle/rifle.ysncd@fire");
    authored.occlusion.ray_count = 3;
    let _ = world.insert(emitter, authored);

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    // Three authored coverage rays remain listener->emitter. A fourth reverse center
    // ray is reserved for thickness/layer reconstruction and therefore excludes the emitter.
    assert_eq!(queries.len(), 4);
    assert_eq!(
        queries
            .iter()
            .filter(|query| query.ignore_entity == Some(player.stable_u64()))
            .count(),
        3
    );
    assert_eq!(
        queries
            .iter()
            .filter(|query| query.ignore_entity == Some(emitter.stable_u64()))
            .count(),
        1
    );
    assert_eq!(
        queries
            .iter()
            .map(|query| query.seq)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
}

#[test]
fn acoustic_provider_resolves_partial_blockage_into_continuous_occlusion_observation() {
    let mut world = World::new();
    world.insert_resource(AudioListenerRuntimeState::default());
    world.insert_resource(AcousticMaterialLibrary::new(vec![AcousticMaterialRule {
        material_id: "material.concrete".to_owned(),
        surface_matches: vec!["concrete".to_owned()],
        profile: AcousticMaterialProfile {
            transmission_gain: 0.16,
            reflection_gain: 0.72,
            high_frequency_absorption: 0.92,
            low_pass_hz: 1_100.0,
        },
    }]));
    let emitter = world.spawn();
    let _ = world.insert(
        emitter,
        Transform {
            position: Vec3::new(0.0, 0.0, -10.0),
            ..Transform::default()
        },
    );
    let mut authored = AudioEmitter::new("shared/audio/weapon/rifle/rifle.ysncd@fire");
    authored.occlusion.ray_count = 3;
    let _ = world.insert(emitter, authored);
    let blocker = world.spawn();
    let _ = world.insert(
        blocker,
        PhysicsSurface {
            id: "surface.wall.concrete".to_owned(),
            ..PhysicsSurface::default()
        },
    );

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    assert_eq!(queries.len(), 4);
    let hit = PhysicsQueryHitDto {
        subshape_id: 0,
        hit_index: 0,
        back_face: false,

        seq: queries[0].seq,
        entity: blocker.stable_u64(),
        position: [0.0, 0.0, -4.0],
        normal: [0.0, 0.0, 1.0],
        distance: 4.0,
    };
    let key_to_entity = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    let consumed = provider.resolve_query_hits(&mut world, 42, &[hit], &key_to_entity);
    assert_eq!(consumed.len(), 4);
    let observation = world
        .get::<AudioOcclusionObservation>(emitter)
        .cloned()
        .expect("raw acoustic observation");
    assert_eq!(observation.fixed_tick, 42);
    assert_eq!(observation.samples, 3);
    assert_eq!(observation.blocked_samples, 1);
    assert!((observation.obstruction - 1.0 / 3.0).abs() < 1.0e-6);
    assert!(observation.occlusion > observation.obstruction);
    assert!(observation.occlusion < 1.0);
    assert_eq!(observation.dominant_material, "material.concrete");
    assert!(observation.material.high_frequency_absorption > 0.9);
    assert!(observation.material.low_pass_hz < 1_200.0);
}

#[test]
fn acoustic_provider_marks_all_blocked_rays_as_full_occlusion() {
    let mut world = World::new();
    world.insert_resource(AudioListenerRuntimeState::default());
    let emitter = world.spawn();
    let _ = world.insert(
        emitter,
        Transform {
            position: Vec3::new(0.0, 0.0, -10.0),
            ..Transform::default()
        },
    );
    let mut authored = AudioEmitter::new("shared/audio/weapon/rifle/rifle.ysncd@fire");
    authored.occlusion.ray_count = 3;
    let _ = world.insert(emitter, authored);
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
    let key_to_entity = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    provider.resolve_query_hits(&mut world, 77, &hits, &key_to_entity);
    let observation = world
        .get::<AudioOcclusionObservation>(emitter)
        .cloned()
        .expect("full occlusion observation");
    assert_eq!(observation.blocked_samples, 3);
    assert_eq!(observation.obstruction, 1.0);
    assert_eq!(observation.occlusion, 1.0);
}

#[test]
fn acoustic_provider_treats_emitter_endpoint_hits_as_clear() {
    let mut world = World::new();
    world.insert_resource(AudioListenerRuntimeState::default());
    let emitter = world.spawn();
    let _ = world.insert(
        emitter,
        Transform {
            position: Vec3::new(0.0, 0.0, -10.0),
            ..Transform::default()
        },
    );
    let mut authored = AudioEmitter::new("shared/audio/weapon/rifle/rifle.ysncd@fire");
    authored.occlusion.ray_count = 3;
    let _ = world.insert(emitter, authored);

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    let hits = queries
        .iter()
        .map(|query| PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: query.seq,
            entity: emitter.stable_u64(),
            position: [0.0, 0.0, -9.9],
            normal: [0.0, 0.0, 1.0],
            distance: 9.9,
        })
        .collect::<Vec<_>>();
    let key_to_entity = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    provider.resolve_query_hits(&mut world, 88, &hits, &key_to_entity);
    let observation = world
        .get::<AudioOcclusionObservation>(emitter)
        .cloned()
        .expect("endpoint observation");
    assert_eq!(observation.blocked_samples, 0);
    assert_eq!(observation.obstruction, 0.0);
    assert_eq!(observation.occlusion, 0.0);
}
#[test]
fn authored_acoustic_surface_override_wins_over_physics_surface_fallback() {
    let mut world = World::new();
    world.insert_resource(AudioListenerRuntimeState::default());

    let emitter = world.spawn();
    let _ = world.insert(
        emitter,
        Transform {
            position: Vec3::new(0.0, 0.0, -10.0),
            ..Transform::default()
        },
    );
    let mut authored_emitter = AudioEmitter::new("shared/audio/weapon/rifle/rifle.ysncd@fire");
    authored_emitter.occlusion.ray_count = 1;
    let _ = world.insert(emitter, authored_emitter);

    let blocker = world.spawn();
    let _ = world.insert(
        blocker,
        PhysicsSurface {
            id: "surface.wall.concrete".to_owned(),
            ..PhysicsSurface::default()
        },
    );
    let _ = world.insert(
        blocker,
        AcousticSurface::new(
            "material.glass.thin",
            AcousticMaterialProfile {
                transmission_gain: 0.52,
                reflection_gain: 0.61,
                high_frequency_absorption: 0.38,
                low_pass_hz: 7_000.0,
            },
        ),
    );

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    assert_eq!(queries.len(), 2);
    let hit = PhysicsQueryHitDto {
        subshape_id: 0,
        hit_index: 0,
        back_face: false,

        seq: queries[0].seq,
        entity: blocker.stable_u64(),
        position: [0.0, 0.0, -4.0],
        normal: [0.0, 0.0, 1.0],
        distance: 4.0,
    };
    let key_to_entity = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    provider.resolve_query_hits(&mut world, 101, &[hit], &key_to_entity);

    let observation = world
        .get::<AudioOcclusionObservation>(emitter)
        .cloned()
        .expect("authored acoustic observation");
    assert_eq!(observation.dominant_material, "material.glass.thin");
    assert!((observation.material.transmission_gain - 0.52).abs() < 1.0e-6);
    assert!((observation.material.high_frequency_absorption - 0.38).abs() < 1.0e-6);
    assert!((observation.material.low_pass_hz - 7_000.0).abs() < 1.0e-3);
}

fn insert_environment_zone(
    world: &mut World,
    id: &str,
    center: Vec3,
    send_gain: f32,
    preset: AudioReverbPreset,
) -> u64 {
    let entity = world.spawn();
    let _ = world.insert(
        entity,
        Transform {
            position: center,
            ..Transform::default()
        },
    );
    let _ = world.insert(
        entity,
        AudioEnvironmentZone {
            zone_id: id.to_owned(),
            half_extents: [5.0, 5.0, 5.0],
            blend_distance: 0.0,
            send_gain,
            reverb: preset,
            ..AudioEnvironmentZone::default()
        },
    );
    entity.stable_u64()
}

#[test]
fn same_environment_zone_uses_one_listener_room_send_without_double_reverb() {
    let mut world = World::new();
    let room_key = insert_environment_zone(
        &mut world,
        "room.concrete",
        Vec3::ZERO,
        0.6,
        AudioReverbPreset::concrete_hall(),
    );
    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    assert_eq!(frame.zone_count(), 1);
    assert_eq!(frame.listener_zone_id(), Some("room.concrete"));
    let resolved = frame.resolve([1.0, 0.0, 0.0]);
    assert_eq!(resolved.emitter_zone, "room.concrete");
    assert_eq!(resolved.listener_zone, "room.concrete");
    assert_eq!(resolved.portal_gain, 1.0);
    assert_eq!(resolved.state.source_send.gain, 0.0);
    assert_eq!(resolved.state.source_send.room_bus_id, 0);
    assert_eq!(resolved.state.listener_send.room_bus_id, room_key);
    assert!((resolved.state.listener_send.gain - 0.6).abs() < 1.0e-6);
    assert!((resolved.state.listener_send.preset.decay_seconds - 2.8).abs() < 1.0e-6);
}

#[test]
fn cross_room_reverb_sends_follow_strongest_portal_gain() {
    let mut world = World::new();
    let room_a_key = insert_environment_zone(
        &mut world,
        "room.a",
        Vec3::ZERO,
        0.6,
        AudioReverbPreset::room(),
    );
    let room_b_key = insert_environment_zone(
        &mut world,
        "room.b",
        Vec3::new(20.0, 0.0, 0.0),
        0.5,
        AudioReverbPreset::metal_hangar(),
    );
    let portal_entity = world.spawn();
    let mut portal = AudioPortal::new("door.a-b", "room.a", "room.b");
    portal.openness = 0.5;
    portal.transmission_gain = 0.8;
    portal.send_gain = 0.75;
    let _ = world.insert(portal_entity, portal);

    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    assert_eq!(frame.portal_count(), 1);
    let resolved = frame.resolve([20.0, 0.0, 0.0]);
    assert_eq!(resolved.emitter_zone, "room.b");
    assert_eq!(resolved.listener_zone, "room.a");
    assert!((resolved.portal_gain - 0.3).abs() < 1.0e-6);
    assert_eq!(resolved.state.source_send.room_bus_id, room_b_key);
    assert_eq!(resolved.state.listener_send.room_bus_id, room_a_key);
    assert_ne!(
        resolved.state.source_send.room_bus_id,
        resolved.state.listener_send.room_bus_id
    );
    assert!((resolved.state.source_send.gain - 0.15).abs() < 1.0e-6);
    assert!((resolved.state.listener_send.gain - 0.18).abs() < 1.0e-6);
    assert_eq!(
        resolved.state.source_send.preset,
        AudioReverbPreset::metal_hangar()
    );
    assert_eq!(
        resolved.state.listener_send.preset,
        AudioReverbPreset::room()
    );
}

#[test]
fn geometric_portal_on_direct_line_has_negligible_detour_delay() {
    let mut world = World::new();
    insert_environment_zone(
        &mut world,
        "room.a",
        Vec3::ZERO,
        0.5,
        AudioReverbPreset::room(),
    );
    insert_environment_zone(
        &mut world,
        "room.b",
        Vec3::new(20.0, 0.0, 0.0),
        0.5,
        AudioReverbPreset::room(),
    );
    let portal_entity = world.spawn();
    let _ = world.insert(
        portal_entity,
        Transform {
            position: Vec3::new(10.0, 0.0, 0.0),
            ..Transform::default()
        },
    );
    let mut portal = AudioPortal::new("door.direct", "room.a", "room.b");
    portal.half_extents = [1.0, 1.2];
    let _ = world.insert(portal_entity, portal);

    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    let resolved = frame.resolve([20.0, 0.0, 0.0]);
    assert_eq!(resolved.portal_gain, 1.0);
    assert!(resolved.state.direct_path.gain > 0.95);
    assert!(resolved.state.direct_path.extra_delay_ms < 1.0e-4);
    assert!(resolved.state.direct_path.high_frequency_gain > 0.85);
}

#[test]
fn portal_direct_path_is_independent_from_indirect_send_gain() {
    let mut world = World::new();
    insert_environment_zone(
        &mut world,
        "room.a",
        Vec3::ZERO,
        0.5,
        AudioReverbPreset::room(),
    );
    insert_environment_zone(
        &mut world,
        "room.b",
        Vec3::new(20.0, 0.0, 0.0),
        0.5,
        AudioReverbPreset::room(),
    );
    let portal_entity = world.spawn();
    let _ = world.insert(
        portal_entity,
        Transform {
            position: Vec3::new(10.0, 0.0, 0.0),
            ..Transform::default()
        },
    );
    let mut portal = AudioPortal::new("door.dry-only", "room.a", "room.b");
    portal.send_gain = 0.0;
    let _ = world.insert(portal_entity, portal);

    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    let resolved = frame.resolve([20.0, 0.0, 0.0]);
    assert_eq!(resolved.portal_gain, 0.0);
    assert_eq!(resolved.state.source_send.gain, 0.0);
    assert_eq!(resolved.state.listener_send.gain, 0.0);
    assert!(resolved.state.direct_path.gain > 0.9);
}

#[test]
fn off_axis_portal_produces_diffracted_delay_and_spectral_loss() {
    let mut world = World::new();
    insert_environment_zone(
        &mut world,
        "room.a",
        Vec3::ZERO,
        0.5,
        AudioReverbPreset::room(),
    );
    insert_environment_zone(
        &mut world,
        "room.b",
        Vec3::new(20.0, 0.0, 0.0),
        0.5,
        AudioReverbPreset::room(),
    );
    let portal_entity = world.spawn();
    let _ = world.insert(
        portal_entity,
        Transform {
            position: Vec3::new(10.0, 0.0, 8.0),
            ..Transform::default()
        },
    );
    let mut portal = AudioPortal::new("door.detour", "room.a", "room.b");
    portal.half_extents = [0.55, 1.0];
    let _ = world.insert(portal_entity, portal);

    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    let resolved = frame.resolve([20.0, 0.0, 0.0]);
    assert!(resolved.state.direct_path.extra_delay_ms > 10.0);
    assert!(resolved.state.direct_path.gain < 0.5);
    assert!(resolved.state.direct_path.high_frequency_gain < 0.2);
    assert!(resolved.state.direct_path.low_pass_hz < 10_000.0);
}

#[test]
fn room_geometry_controls_first_order_reflection_timing() {
    let mut world = World::new();
    insert_environment_zone(
        &mut world,
        "room.geometry",
        Vec3::ZERO,
        0.6,
        AudioReverbPreset::room(),
    );
    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    let centered = frame.resolve([1.0, 0.0, 0.0]);
    let near_wall = frame.resolve([4.5, 0.0, 0.0]);
    assert!(
        near_wall.state.listener_send.preset.pre_delay_ms
            < centered.state.listener_send.preset.pre_delay_ms
    );
    assert!(
        centered
            .state
            .listener_send
            .preset
            .early_reflections_spread_ms
            > 0.0
    );
}

#[test]
fn closed_portal_removes_cross_room_reverb_path() {
    let mut world = World::new();
    insert_environment_zone(
        &mut world,
        "room.a",
        Vec3::ZERO,
        0.6,
        AudioReverbPreset::room(),
    );
    insert_environment_zone(
        &mut world,
        "room.b",
        Vec3::new(20.0, 0.0, 0.0),
        0.5,
        AudioReverbPreset::corridor(),
    );
    let portal_entity = world.spawn();
    let mut portal = AudioPortal::new("door.closed", "room.a", "room.b");
    portal.openness = 0.0;
    let _ = world.insert(portal_entity, portal);

    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    assert_eq!(frame.portal_count(), 0);
    let resolved = frame.resolve([20.0, 0.0, 0.0]);
    assert_eq!(resolved.portal_gain, 0.0);
    assert_eq!(resolved.state.source_send.gain, 0.0);
    assert_eq!(resolved.state.listener_send.gain, 0.0);
}

#[test]
fn environment_zone_overlap_is_deterministic_by_priority() {
    let mut world = World::new();
    let low = world.spawn();
    let _ = world.insert(low, Transform::default());
    let _ = world.insert(
        low,
        AudioEnvironmentZone {
            zone_id: "room.low".to_owned(),
            half_extents: [10.0, 10.0, 10.0],
            priority: 0,
            blend_distance: 0.0,
            ..AudioEnvironmentZone::default()
        },
    );
    let high = world.spawn();
    let _ = world.insert(
        high,
        Transform {
            position: Vec3::new(4.0, 0.0, 0.0),
            ..Transform::default()
        },
    );
    let _ = world.insert(
        high,
        AudioEnvironmentZone {
            zone_id: "room.high".to_owned(),
            half_extents: [10.0, 10.0, 10.0],
            priority: 10,
            blend_distance: 0.0,
            ..AudioEnvironmentZone::default()
        },
    );
    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    assert_eq!(frame.listener_zone_id(), Some("room.high"));
}

#[test]
fn rotated_environment_zone_uses_oriented_box_membership() {
    let mut world = World::new();
    let zone_entity = world.spawn();
    let _ = world.insert(
        zone_entity,
        Transform {
            rotation: Quat::from_rotation_y(core::f32::consts::FRAC_PI_2),
            ..Transform::default()
        },
    );
    let _ = world.insert(
        zone_entity,
        AudioEnvironmentZone {
            zone_id: "corridor.rotated".to_owned(),
            half_extents: [6.0, 2.0, 1.0],
            blend_distance: 0.0,
            reverb: AudioReverbPreset::corridor(),
            ..AudioEnvironmentZone::default()
        },
    );
    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::new(0.0, 0.0, 5.0));
    assert_eq!(frame.listener_zone_id(), Some("corridor.rotated"));
}

#[test]
fn multi_hop_portal_graph_uses_strongest_product_route() {
    let mut world = World::new();
    insert_environment_zone(
        &mut world,
        "room.a",
        Vec3::ZERO,
        0.6,
        AudioReverbPreset::room(),
    );
    insert_environment_zone(
        &mut world,
        "room.b",
        Vec3::new(20.0, 0.0, 0.0),
        0.5,
        AudioReverbPreset::corridor(),
    );
    insert_environment_zone(
        &mut world,
        "room.c",
        Vec3::new(40.0, 0.0, 0.0),
        0.7,
        AudioReverbPreset::concrete_hall(),
    );

    let ab_entity = world.spawn();
    let mut ab = AudioPortal::new("portal.ab", "room.a", "room.b");
    ab.openness = 0.8;
    let _ = world.insert(ab_entity, ab);

    let bc_entity = world.spawn();
    let mut bc = AudioPortal::new("portal.bc", "room.b", "room.c");
    bc.openness = 0.5;
    let _ = world.insert(bc_entity, bc);

    let ac_entity = world.spawn();
    let mut ac = AudioPortal::new("portal.ac", "room.a", "room.c");
    ac.openness = 0.3;
    let _ = world.insert(ac_entity, ac);

    let frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    let resolved = frame.resolve([40.0, 0.0, 0.0]);
    assert!((resolved.portal_gain - 0.4).abs() < 1.0e-6);
    assert!((resolved.state.source_send.gain - 0.28).abs() < 1.0e-6);
    assert!((resolved.state.listener_send.gain - 0.24).abs() < 1.0e-6);
}

#[test]
fn environment_without_listener_state_fails_open_instead_of_assuming_world_origin() {
    let mut world = World::new();
    insert_environment_zone(
        &mut world,
        "room.origin",
        Vec3::ZERO,
        0.8,
        AudioReverbPreset::concrete_hall(),
    );
    let frame = AudioEnvironmentFrame::snapshot(&world);
    assert_eq!(frame.listener_zone_id(), None);
    let resolved = frame.resolve([0.0, 0.0, 0.0]);
    assert_eq!(resolved.state.source_send.gain, 0.0);
    assert_eq!(resolved.state.listener_send.gain, 0.0);
    assert_eq!(resolved.portal_gain, 0.0);
}

#[test]
fn ambience_beds_follow_indoor_outdoor_and_portal_zone_state() {
    let mut world = World::new();
    let indoor_entity = world.spawn();
    let _ = world.insert(
        indoor_entity,
        Transform {
            position: Vec3::ZERO,
            ..Transform::default()
        },
    );
    let _ = world.insert(
        indoor_entity,
        AudioEnvironmentZone {
            zone_id: "room.indoor".to_owned(),
            kind: AudioEnvironmentKind::Indoor,
            half_extents: [5.0, 5.0, 5.0],
            blend_distance: 0.0,
            ..AudioEnvironmentZone::default()
        },
    );

    let outdoor_entity = world.spawn();
    let _ = world.insert(
        outdoor_entity,
        Transform {
            position: Vec3::new(20.0, 0.0, 0.0),
            ..Transform::default()
        },
    );
    let _ = world.insert(
        outdoor_entity,
        AudioEnvironmentZone {
            zone_id: "yard.outdoor".to_owned(),
            kind: AudioEnvironmentKind::Outdoor,
            half_extents: [5.0, 5.0, 5.0],
            blend_distance: 0.0,
            ..AudioEnvironmentZone::default()
        },
    );

    let portal_entity = world.spawn();
    let mut portal = AudioPortal::new("door.indoor-yard", "room.indoor", "yard.outdoor");
    portal.openness = 0.5;
    let _ = world.insert(portal_entity, portal);

    let indoor_frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::ZERO);
    assert!(!indoor_frame.listener_is_outdoor());
    let mut indoor = AudioAmbienceBed::new("inside", "shared/audio/inside.ogg");
    indoor.scope = AudioAmbienceScope::Indoor;
    assert_eq!(
        AudioAmbienceRuntimeModule::target_activation(&indoor, &indoor_frame),
        1.0
    );

    let mut outdoor = AudioAmbienceBed::new("outside", "shared/audio/outside.ogg");
    outdoor.scope = AudioAmbienceScope::Outdoor;
    assert_eq!(
        AudioAmbienceRuntimeModule::target_activation(&outdoor, &indoor_frame),
        0.0
    );

    let mut yard = AudioAmbienceBed::new("yard", "shared/audio/yard.ogg");
    yard.scope = AudioAmbienceScope::Zones;
    yard.zones = vec!["yard.outdoor".to_owned()];
    yard.portal_bleed = 0.4;
    assert!(
        (AudioAmbienceRuntimeModule::target_activation(&yard, &indoor_frame) - 0.2).abs() < 1.0e-6
    );

    let outdoor_frame = AudioEnvironmentFrame::snapshot_at(&world, Vec3::new(20.0, 0.0, 0.0));
    assert!(outdoor_frame.listener_is_outdoor());
    assert_eq!(
        AudioAmbienceRuntimeModule::target_activation(&outdoor, &outdoor_frame),
        1.0
    );
    assert_eq!(
        AudioAmbienceRuntimeModule::target_activation(&yard, &outdoor_frame),
        1.0
    );
}

#[test]
fn ambience_without_listener_snapshot_stays_inactive() {
    let world = World::new();
    let frame = AudioEnvironmentFrame::snapshot(&world);
    let bed = AudioAmbienceBed::new("global", "shared/audio/global.ogg");
    assert!(!frame.listener_ready());
    assert_eq!(
        AudioAmbienceRuntimeModule::target_activation(&bed, &frame),
        0.0
    );
}

#[test]
fn reflection_visibility_contract_reaches_directional_environment_send() {
    let mut world = World::new();
    world.insert_resource(AudioListenerRuntimeState::default());

    let room = world.spawn();
    let _ = world.insert(room, Transform::default());
    let _ = world.insert(
        room,
        AudioEnvironmentZone {
            zone_id: "room.conformance".to_owned(),
            kind: AudioEnvironmentKind::Indoor,
            half_extents: [5.0, 4.0, 6.0],
            blend_distance: 0.0,
            send_gain: 0.7,
            reverb: AudioReverbPreset::room(),
            ..AudioEnvironmentZone::default()
        },
    );

    let emitter = world.spawn();
    let _ = world.insert(
        emitter,
        Transform {
            position: Vec3::new(1.0, 0.0, 0.0),
            ..Transform::default()
        },
    );
    let _ = world.insert(
        emitter,
        AudioEmitter::new("shared/audio/weapon/rifle/rifle.ysncd@fire"),
    );

    let provider = AudioReflectionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    assert_eq!(queries.len(), 24);
    let key_to_entity = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    let consumed = provider.resolve_query_hits(&mut world, 42, &[], &key_to_entity);
    assert_eq!(consumed.len(), 24);

    let observation = world
        .get::<AudioEarlyReflectionObservation>(emitter)
        .expect("reflection observation");
    assert_eq!(observation.paths.len(), 6);
    assert_eq!(observation.second_order_paths.len(), 4);
    assert!(observation.paths.iter().all(|path| path.visible));
    assert!(observation
        .second_order_paths
        .iter()
        .all(|path| path.visible));

    let frame = AudioEnvironmentFrame::snapshot(&world);
    let resolved = frame.resolve_for_emitter(emitter.stable_u64(), [1.0, 0.0, 0.0]);
    let direction = resolved.state.listener_send.early_reflection_direction;
    let length =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();
    assert!((length - 1.0).abs() < 1.0e-4);
    assert!(resolved.state.listener_send.preset.early_reflections_gain > 0.0);
    let early = resolved.state.listener_send.early_reflections;
    assert!(early.active().iter().any(|tap| tap.order == 1));
    assert!(early.active().iter().any(|tap| tap.order == 2));
}

#[test]
fn proven_occlusion_blocker_drives_only_its_diffraction_edge_graph() {
    let mut world = World::new();
    world.insert_resource(AudioListenerRuntimeState {
        listener: newengine_audio_api::AudioListenerState {
            position: [0.0, 0.0, 0.0],
            ..Default::default()
        },
        listener_entity: None,
        frame_index: 1,
    });

    let blocker = world.spawn();
    let _ = world.insert(
        blocker,
        Transform {
            position: Vec3::new(2.0, 0.0, 0.0),
            ..Transform::default()
        },
    );
    let vertices = vec![
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];
    let triangles = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    let _ = world.insert(
        blocker,
        StaticMeshCollider::new(vertices, triangles).expect("cube collider"),
    );

    let unrelated = world.spawn();
    let _ = world.insert(
        unrelated,
        Transform {
            position: Vec3::new(20.0, 0.0, 0.0),
            ..Transform::default()
        },
    );
    let unrelated_vertices = vec![
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ];
    let unrelated_triangles = vec![[0, 1, 2], [0, 2, 3]];
    let _ = world.insert(
        unrelated,
        StaticMeshCollider::new(unrelated_vertices, unrelated_triangles)
            .expect("unrelated collider"),
    );

    let emitter = world.spawn();
    let _ = world.insert(
        emitter,
        Transform {
            position: Vec3::new(4.0, 0.0, 0.0),
            ..Transform::default()
        },
    );
    let _ = world.insert(emitter, AudioEmitter::new("shared/audio/test.ysncd@edge"));
    let _ = world.insert(
        emitter,
        AudioOcclusionObservation {
            fixed_tick: 7,
            samples: 3,
            blocked_samples: 3,
            obstruction: 1.0,
            occlusion: 1.0,
            estimated_thickness_m: 1.0,
            center_blocker_layers: 1,
            dominant_blocker_entity: Some(blocker.stable_u64()),
            dominant_material: "surface.default".to_owned(),
            material: AcousticMaterialProfile::transparent(),
        },
    );

    let provider = AudioDiffractionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    assert!(!queries.is_empty());
    assert!(
        queries.len() <= 12,
        "six bounded edges, two visibility legs each"
    );

    let key_to_entity = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();
    let consumed = provider.resolve_query_hits(&mut world, 8, &[], &key_to_entity);
    assert_eq!(consumed.len(), queries.len());

    let observation = world
        .get::<AudioEdgeDiffractionObservation>(emitter)
        .expect("diffraction observation");
    assert_eq!(observation.blocker_entity, Some(blocker.stable_u64()));
    assert!(!observation.paths.is_empty());
    assert!(observation.paths.iter().all(|path| path.visible));
}
