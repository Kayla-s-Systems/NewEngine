use newengine_audio_api::{
    AcousticMaterialProfile, AudioAmbienceBed, AudioAmbienceScope, AudioEnvironmentKind,
    AudioEnvironmentZone, AudioPortal, AudioReverbPreset,
};
use newengine_camera_contracts::CameraFrameSnapshot;
use newengine_ecs::World;
use newengine_engine_runtime::audio_gateway::audio_listener_from_camera_snapshot;
use newengine_engine_runtime::gameplay::{
    spawn_default_player, GameplayPhysicsQueryProvider, PhysicsSurface,
};
use newengine_engine_runtime::{
    AcousticSurface, AudioAmbienceRuntimeModule, AudioEmitter, AudioEnvironmentFrame,
    AudioListenerRuntimeState, AudioOcclusionObservation, AudioOcclusionPhysicsQueryProvider,
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
fn authored_audio_emitter_references_yscd_cue_not_backend_clip() {
    let emitter = AudioEmitter::new("shared/audio/weapon/rifle/rifle.yscd@fire");
    assert_eq!(emitter.cue, "shared/audio/weapon/rifle/rifle.yscd@fire");
    assert!(emitter.enabled);
    assert!(emitter.autoplay);
    assert!(emitter.spatial);
}

#[test]
fn acoustic_provider_batches_multi_ray_queries_and_explicitly_ignores_listener_player() {
    let mut world = World::new();
    world.insert_resource(AudioListenerRuntimeState::default());
    let player = spawn_default_player(&mut world, None, "audio-listener-player", Vec3::ZERO);
    let emitter = world.spawn();
    let _ = world.insert(
        emitter,
        Transform {
            position: Vec3::new(0.0, 0.0, -12.0),
            ..Transform::default()
        },
    );
    let mut authored = AudioEmitter::new("shared/audio/weapon/rifle/rifle.yscd@fire");
    authored.occlusion.ray_count = 3;
    let _ = world.insert(emitter, authored);

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    assert_eq!(queries.len(), 3);
    assert!(queries
        .iter()
        .all(|query| query.ignore_entity == Some(player.stable_u64())));
    assert_eq!(
        queries
            .iter()
            .map(|query| query.seq)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
}

#[test]
fn acoustic_provider_resolves_partial_blockage_into_raw_obstruction_observation() {
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
    let mut authored = AudioEmitter::new("shared/audio/weapon/rifle/rifle.yscd@fire");
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
    assert_eq!(queries.len(), 3);
    let hit = PhysicsQueryHitDto {
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
    assert_eq!(consumed.len(), 3);
    let observation = world
        .get::<AudioOcclusionObservation>(emitter)
        .cloned()
        .expect("raw acoustic observation");
    assert_eq!(observation.fixed_tick, 42);
    assert_eq!(observation.samples, 3);
    assert_eq!(observation.blocked_samples, 1);
    assert!((observation.obstruction - 1.0 / 3.0).abs() < 1.0e-6);
    assert_eq!(observation.occlusion, 0.0);
    assert_eq!(observation.dominant_material, "surface.wall.concrete");
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
    let mut authored = AudioEmitter::new("shared/audio/weapon/rifle/rifle.yscd@fire");
    authored.occlusion.ray_count = 3;
    let _ = world.insert(emitter, authored);
    let blocker = world.spawn();

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    let hits = queries
        .iter()
        .map(|query| PhysicsQueryHitDto {
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
    let mut authored = AudioEmitter::new("shared/audio/weapon/rifle/rifle.yscd@fire");
    authored.occlusion.ray_count = 3;
    let _ = world.insert(emitter, authored);

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    let hits = queries
        .iter()
        .map(|query| PhysicsQueryHitDto {
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
    let mut authored_emitter = AudioEmitter::new("shared/audio/weapon/rifle/rifle.yscd@fire");
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
                high_frequency_absorption: 0.38,
                low_pass_hz: 7_000.0,
            },
        ),
    );

    let provider = AudioOcclusionPhysicsQueryProvider::new();
    let queries = provider.collect_queries(&world);
    assert_eq!(queries.len(), 1);
    let hit = PhysicsQueryHitDto {
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
) {
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
}

#[test]
fn same_environment_zone_uses_one_listener_room_send_without_double_reverb() {
    let mut world = World::new();
    insert_environment_zone(
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
    assert!((resolved.state.listener_send.gain - 0.6).abs() < 1.0e-6);
    assert!((resolved.state.listener_send.preset.decay_seconds - 2.8).abs() < 1.0e-6);
}

#[test]
fn cross_room_reverb_sends_follow_strongest_portal_gain() {
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
