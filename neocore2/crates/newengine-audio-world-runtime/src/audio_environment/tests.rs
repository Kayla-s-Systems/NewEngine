use super::*;
use newengine_audio_api::AudioReverbPreset;

fn zone(
    key: u64,
    id: &str,
    center: Vec3,
    half_extents: Vec3,
    priority: i32,
) -> ResolvedEnvironmentZone {
    ResolvedEnvironmentZone {
        stable_key: key,
        zone: AudioEnvironmentZone {
            zone_id: id.to_owned(),
            priority,
            half_extents: [half_extents.x, half_extents.y, half_extents.z],
            blend_distance: 0.0,
            send_gain: 0.5,
            reverb: AudioReverbPreset::room(),
            ..AudioEnvironmentZone::default()
        },
        center,
        rotation: Quat::IDENTITY,
        half_extents,
    }
}

#[test]
fn overlapping_zone_selection_prefers_priority_then_center_distance() {
    let zones = vec![
        zone(1, "room.low", Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0), 0),
        zone(
            2,
            "room.high",
            Vec3::new(5.0, 0.0, 0.0),
            Vec3::new(10.0, 10.0, 10.0),
            5,
        ),
    ];
    let selected = select_membership(&zones, Vec3::ZERO).expect("membership");
    assert_eq!(zones[selected.zone_index].zone.zone_id, "room.high");

    let equal_priority = vec![
        zone(
            3,
            "room.far",
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(10.0, 10.0, 10.0),
            5,
        ),
        zone(4, "room.near", Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0), 5),
    ];
    let selected = select_membership(&equal_priority, Vec3::ZERO).expect("membership");
    assert_eq!(
        equal_priority[selected.zone_index].zone.zone_id,
        "room.near"
    );
}

#[test]
fn room_geometry_moves_first_order_early_reflections() {
    let room = zone(10, "room.geometry", Vec3::ZERO, Vec3::new(5.0, 3.0, 7.0), 0);
    let centered = geometry_adjusted_reverb(
        &room,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        AudioReverbPreset::room(),
        None,
    );
    let near_wall = geometry_adjusted_reverb(
        &room,
        Vec3::new(4.5, 0.0, 0.0),
        Vec3::new(3.5, 0.0, 0.0),
        AudioReverbPreset::room(),
        None,
    );
    assert!(near_wall.pre_delay_ms < centered.pre_delay_ms);
    assert!(centered.early_reflections_spread_ms > 0.0);
    assert!(near_wall.early_reflections_spread_ms > 0.0);
}

#[test]
fn portal_detour_adds_delay_and_high_frequency_loss() {
    let portal = AudioPortal::new("door", "a", "b");
    let straight = direct_portal_response(
        &portal,
        Vec3::ZERO,
        Vec3::new(-5.0, 0.0, 0.0),
        Vec3::new(5.0, 0.0, 0.0),
    );
    let detour = direct_portal_response(
        &portal,
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::new(-5.0, 0.0, 0.0),
        Vec3::new(5.0, 0.0, 0.0),
    );
    assert!(straight.extra_delay_ms < 1.0e-4);
    assert!(detour.extra_delay_ms > 10.0);
    assert!(detour.gain < straight.gain);
    assert!(detour.high_frequency_gain < straight.high_frequency_gain);
    assert!(detour.low_pass_hz < straight.low_pass_hz);
}

#[test]
fn smaller_portal_aperture_diffracts_high_frequencies_more() {
    let mut wide = AudioPortal::new("wide", "a", "b");
    wide.half_extents = [1.5, 1.5];
    let mut narrow = AudioPortal::new("narrow", "a", "b");
    narrow.half_extents = [0.12, 0.9];
    let emitter = Vec3::new(-5.0, 0.0, 0.0);
    let listener = Vec3::new(5.0, 0.0, 0.0);
    let center = Vec3::new(0.0, 0.0, 2.5);
    let wide_response = direct_portal_response(&wide, center, emitter, listener);
    let narrow_response = direct_portal_response(&narrow, center, emitter, listener);
    assert!(narrow_response.high_frequency_gain < wide_response.high_frequency_gain);
    assert!(narrow_response.low_pass_hz < wide_response.low_pass_hz);
    assert!(narrow_response.gain < wide_response.gain);
}

#[test]
fn strongest_portal_path_multiplies_edges_and_prefers_better_route() {
    let zones = vec![
        zone(1, "a", Vec3::ZERO, Vec3::ONE, 0),
        zone(2, "b", Vec3::ZERO, Vec3::ONE, 0),
        zone(3, "c", Vec3::ZERO, Vec3::ONE, 0),
    ];
    let mut ab = AudioPortal::new("ab", "a", "b");
    ab.openness = 0.8;
    let mut bc = AudioPortal::new("bc", "b", "c");
    bc.openness = 0.5;
    let mut ac = AudioPortal::new("ac", "a", "c");
    ac.openness = 0.3;
    let gains = strongest_portal_routes(&zones, &[ab, bc, ac], 0);
    assert!((gains["c"] - 0.4).abs() < 1.0e-6);
}

#[test]
fn multi_hop_geometric_route_accumulates_waypoint_delay_and_multi_edge_diffraction() {
    let zones = vec![
        zone(1, "a", Vec3::ZERO, Vec3::ONE, 0),
        zone(2, "b", Vec3::ZERO, Vec3::ONE, 0),
        zone(3, "c", Vec3::ZERO, Vec3::ONE, 0),
    ];
    let mut ab = AudioPortal::new("ab", "a", "b");
    ab.openness = 0.9;
    ab.half_extents = [0.15, 0.9];
    let mut bc = AudioPortal::new("bc", "b", "c");
    bc.openness = 0.9;
    bc.half_extents = [0.15, 0.9];
    let portals = vec![ab.clone(), bc.clone()];
    let routes = strongest_direct_portal_routes(&zones, &portals, 0);
    let route = routes.get("c").expect("multi-hop direct route");
    assert_eq!(route.portal_ids, vec!["ab".to_owned(), "bc".to_owned()]);
    assert!((route.gain - 0.81).abs() < 1.0e-6);

    let centers = BTreeMap::from([
        ("ab".to_owned(), Vec3::new(-2.0, 0.0, 3.0)),
        ("bc".to_owned(), Vec3::new(2.0, 0.0, -3.0)),
    ]);
    let emitter = Vec3::new(5.0, 0.0, 0.0);
    let listener = Vec3::new(-5.0, 0.0, 0.0);
    let (narrow, source_boundary, listener_boundary) =
        direct_portal_route_response(route, &portals, &centers, emitter, listener)
            .expect("geometric route");
    assert_eq!(source_boundary, Some(centers["bc"]));
    assert_eq!(listener_boundary, Some(centers["ab"]));
    assert!(narrow.extra_delay_ms > 10.0);
    assert!(narrow.gain < route.gain);
    assert!(narrow.high_frequency_gain < 1.0);

    // Widening only one of two apertures must improve the result, proving that each edge
    // contributes independently instead of collapsing the chain into one scalar portal gain.
    let mut widened_ab = ab;
    widened_ab.half_extents = [1.5, 1.5];
    let partly_wide_portals = vec![widened_ab, bc];
    let (partly_wide, _, _) =
        direct_portal_route_response(route, &partly_wide_portals, &centers, emitter, listener)
            .expect("partly wide geometric route");
    assert!((partly_wide.extra_delay_ms - narrow.extra_delay_ms).abs() < 1.0e-4);
    assert!(partly_wide.high_frequency_gain > narrow.high_frequency_gain);
    assert!(partly_wide.gain > narrow.gain);
}

#[test]
fn direct_and_indirect_graphs_can_choose_different_portal_topology() {
    let zones = vec![
        zone(1, "a", Vec3::ZERO, Vec3::ONE, 0),
        zone(2, "b", Vec3::ZERO, Vec3::ONE, 0),
        zone(3, "c", Vec3::ZERO, Vec3::ONE, 0),
    ];
    let mut ab = AudioPortal::new("ab", "a", "b");
    ab.openness = 0.95;
    ab.send_gain = 0.10;
    let mut bc = AudioPortal::new("bc", "b", "c");
    bc.openness = 0.95;
    bc.send_gain = 0.10;
    let mut ac = AudioPortal::new("ac", "a", "c");
    ac.openness = 0.70;
    ac.send_gain = 1.0;
    let portals = vec![ab, bc, ac];

    let direct = strongest_portal_route_map(&zones, &portals, 0, PortalRouteMetric::Direct);
    let indirect = strongest_portal_route_map(&zones, &portals, 0, PortalRouteMetric::Indirect);
    assert_eq!(
        direct["c"].portal_ids,
        vec!["ab".to_owned(), "bc".to_owned()]
    );
    assert_eq!(indirect["c"].portal_ids, vec!["ac".to_owned()]);
    assert!(direct["c"].gain > 0.90);
    assert!((indirect["c"].gain - 0.70).abs() < 1.0e-6);
}

#[test]
fn reflection_visibility_and_material_absorption_shape_early_field() {
    let room = ResolvedEnvironmentZone {
        stable_key: 1,
        zone: AudioEnvironmentZone {
            zone_id: "room.test".to_owned(),
            kind: AudioEnvironmentKind::Indoor,
            half_extents: [5.0, 5.0, 5.0],
            blend_distance: 0.0,
            ..AudioEnvironmentZone::default()
        },
        center: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        half_extents: Vec3::splat(5.0),
    };
    let source = Vec3::ZERO;
    let listener = Vec3::new(1.0, 0.0, 0.0);
    let geometry = newengine_audio_world_api::first_order_reflection_geometry(
        newengine_audio_world_api::AudioRoomObbGeometry {
            center: [0.0; 3],
            rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
            half_extents: [5.0; 3],
        },
        [0.0; 3],
        [1.0, 0.0, 0.0],
    );
    let reflective = newengine_audio_api::AcousticMaterialProfile {
        transmission_gain: 0.2,
        reflection_gain: 0.8,
        high_frequency_absorption: 0.15,
        low_pass_hz: 5_000.0,
    };
    let absorptive = newengine_audio_api::AcousticMaterialProfile {
        transmission_gain: 0.2,
        reflection_gain: 0.35,
        high_frequency_absorption: 0.90,
        low_pass_hz: 2_000.0,
    };
    let make_observation = |blocked_face: Option<u8>, material| AudioEarlyReflectionObservation {
        fixed_tick: 10,
        source_position: [0.0; 3],
        listener_position: [1.0, 0.0, 0.0],
        paths: geometry
            .iter()
            .map(
                |path| newengine_audio_world_api::AudioEarlyReflectionPathObservation {
                    face_index: path.face_index,
                    visible: blocked_face != Some(path.face_index),
                    boundary_entity: Some(42),
                    reflection_point: path.reflection_point,
                    arrival_direction: path.arrival_direction,
                    path_length_m: path.path_length_m,
                    excess_length_m: path.excess_length_m,
                    material_known: true,
                    material,
                },
            )
            .collect(),
        second_order_paths: Vec::new(),
    };
    let open = make_observation(None, reflective);
    let blocked = make_observation(Some(geometry[0].face_index), reflective);
    let soft = make_observation(None, absorptive);
    let open_preset = geometry_adjusted_reverb(
        &room,
        source,
        listener,
        AudioReverbPreset::room(),
        Some(&open),
    );
    let blocked_preset = geometry_adjusted_reverb(
        &room,
        source,
        listener,
        AudioReverbPreset::room(),
        Some(&blocked),
    );
    let soft_preset = geometry_adjusted_reverb(
        &room,
        source,
        listener,
        AudioReverbPreset::room(),
        Some(&soft),
    );
    assert!(blocked_preset.early_reflections_gain < open_preset.early_reflections_gain);
    assert!(soft_preset.early_reflections_gain < open_preset.early_reflections_gain);
    assert!(
        soft_preset.early_reflections_high_frequency_gain
            < open_preset.early_reflections_high_frequency_gain
    );
}

#[test]
fn second_order_reflection_becomes_discrete_later_tap_with_multiplicative_material_loss() {
    let first_material = newengine_audio_api::AcousticMaterialProfile {
        transmission_gain: 1.0,
        reflection_gain: 0.8,
        high_frequency_absorption: 0.10,
        low_pass_hz: 12_000.0,
    };
    let second_a = newengine_audio_api::AcousticMaterialProfile {
        transmission_gain: 1.0,
        reflection_gain: 0.55,
        high_frequency_absorption: 0.35,
        low_pass_hz: 8_000.0,
    };
    let second_b = newengine_audio_api::AcousticMaterialProfile {
        transmission_gain: 1.0,
        reflection_gain: 0.45,
        high_frequency_absorption: 0.50,
        low_pass_hz: 6_000.0,
    };
    let observation = AudioEarlyReflectionObservation {
        fixed_tick: 12,
        source_position: [0.0, 0.0, 0.0],
        listener_position: [1.0, 0.0, 0.0],
        paths: vec![
            newengine_audio_world_api::AudioEarlyReflectionPathObservation {
                face_index: 1,
                visible: true,
                boundary_entity: Some(10),
                reflection_point: [2.0, 0.0, 0.0],
                arrival_direction: [1.0, 0.0, 0.0],
                path_length_m: 2.0,
                excess_length_m: 1.0,
                material_known: true,
                material: first_material,
            },
        ],
        second_order_paths: vec![
            newengine_audio_world_api::AudioSecondOrderReflectionPathObservation {
                face_indices: [1, 3],
                visible: true,
                boundary_entities: [Some(11), Some(12)],
                reflection_points: [[2.0, 0.0, 0.0], [2.0, 2.0, 0.0]],
                arrival_direction: [0.0, 1.0, 0.0],
                path_length_m: 3.0,
                excess_length_m: 2.0,
                material_known: [true, true],
                materials: [second_a, second_b],
            },
        ],
    };
    let field = explicit_early_reflection_field(
        AudioReverbPreset::room(),
        Vec3::ZERO,
        Vec3::new(1.0, 0.0, 0.0),
        Some(&observation),
    );
    assert_eq!(field.count, 2);
    let first = field.active().iter().find(|tap| tap.order == 1).unwrap();
    let second = field.active().iter().find(|tap| tap.order == 2).unwrap();
    assert!(second.delay_ms > first.delay_ms);
    assert!(second.gain < first.gain);
    let expected_hf = second_a.high_frequency_gain() * second_b.high_frequency_gain();
    assert!((second.high_frequency_gain - expected_hf).abs() < 1.0e-6);
    assert_eq!(second.direction, [0.0, 1.0, 0.0]);
}

#[test]
fn stale_reflection_observation_falls_back_to_current_room_geometry() {
    let room = ResolvedEnvironmentZone {
        stable_key: 1,
        zone: AudioEnvironmentZone::new("room.test", [5.0, 5.0, 5.0]),
        center: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        half_extents: Vec3::splat(5.0),
    };
    let stale = AudioEarlyReflectionObservation {
        fixed_tick: 1,
        source_position: [100.0, 0.0, 0.0],
        listener_position: [101.0, 0.0, 0.0],
        paths: vec![
            newengine_audio_world_api::AudioEarlyReflectionPathObservation {
                visible: false,
                ..Default::default()
            },
        ],
        second_order_paths: Vec::new(),
    };
    let current = geometry_adjusted_reverb(
        &room,
        Vec3::ZERO,
        Vec3::new(1.0, 0.0, 0.0),
        AudioReverbPreset::room(),
        Some(&stale),
    );
    assert!(current.early_reflections_gain > 0.0);
}
