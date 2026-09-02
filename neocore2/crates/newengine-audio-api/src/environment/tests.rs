    use super::*;

    #[test]
    fn environment_presets_are_distinct_and_bounded() {
        let room = AudioReverbPreset::room().sanitized();
        let hall = AudioReverbPreset::concrete_hall().sanitized();
        let hangar = AudioReverbPreset::metal_hangar().sanitized();
        assert!(room.decay_seconds < hall.decay_seconds);
        assert!(hall.decay_seconds < hangar.decay_seconds);
        assert!(hangar.damping < room.damping);
    }

    #[test]
    fn portal_gain_tracks_openness_and_transmission() {
        let mut portal = AudioPortal::new("door", "room.a", "room.b");
        portal.openness = 0.5;
        portal.transmission_gain = 0.8;
        portal.send_gain = 0.75;
        assert!((portal.direct_route_gain() - 0.4).abs() < 1.0e-6);
        assert!((portal.route_gain() - 0.3).abs() < 1.0e-6);
        portal.send_gain = 0.0;
        assert!((portal.direct_route_gain() - 0.4).abs() < 1.0e-6);
        assert_eq!(portal.route_gain(), 0.0);
        portal.enabled = false;
        assert_eq!(portal.direct_route_gain(), 0.0);
        assert_eq!(portal.route_gain(), 0.0);
    }

    #[test]
    fn environment_state_smooths_room_transitions() {
        let target = AudioEnvironmentState {
            source_send: AudioReverbSend {
                room_bus_id: 0,
                gain: 0.6,
                preset: AudioReverbPreset::metal_hangar(),
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            listener_send: AudioReverbSend {
                room_bus_id: 0,
                gain: 0.4,
                preset: AudioReverbPreset::room(),
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            direct_path: AudioDirectPathResponse {
                gain: 0.5,
                ..AudioDirectPathResponse::clear()
            },
            portal_gain: 0.5,
        };
        let moved = AudioEnvironmentState::clear().smoothed_towards(target, 0.016, 0.2);
        assert!(moved.source_send.gain > 0.0 && moved.source_send.gain < 0.6);
        assert!(moved.direct_path.gain < 1.0 && moved.direct_path.gain > 0.5);
        assert!(moved.portal_gain < 1.0 && moved.portal_gain > 0.5);
    }

    #[test]
    fn explicit_early_reflection_field_is_bounded_sorted_and_sanitized() {
        let mut field = AudioEarlyReflectionField {
            count: 20,
            ..AudioEarlyReflectionField::default()
        };
        field.taps[0] = AudioEarlyReflectionTap {
            delay_ms: 30.0,
            gain: 0.4,
            high_frequency_gain: 0.8,
            direction: [3.0, 0.0, 0.0],
            order: 1,
        };
        field.taps[1] = AudioEarlyReflectionTap {
            delay_ms: 10.0,
            gain: 0.2,
            high_frequency_gain: 2.0,
            direction: [0.0, 0.0, 0.0],
            order: 9,
        };
        let field = field.sanitized();
        assert_eq!(usize::from(field.count), AUDIO_MAX_EARLY_REFLECTION_TAPS);
        assert_eq!(field.taps[0].delay_ms, 0.0);
        assert!(field
            .active()
            .windows(2)
            .all(|pair| pair[0].delay_ms <= pair[1].delay_ms));
        let delayed = field
            .active()
            .iter()
            .find(|tap| tap.delay_ms == 10.0)
            .unwrap();
        assert_eq!(delayed.high_frequency_gain, 1.0);
        assert_eq!(delayed.order, 2);
        let directional = field
            .active()
            .iter()
            .find(|tap| tap.delay_ms == 30.0)
            .unwrap();
        assert!((directional.direction[0] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn early_reflection_field_fades_topology_changes_without_allocating() {
        let mut target = AudioEarlyReflectionField {
            count: 1,
            ..AudioEarlyReflectionField::default()
        };
        target.taps[0] = AudioEarlyReflectionTap {
            delay_ms: 20.0,
            gain: 0.8,
            high_frequency_gain: 0.5,
            direction: [1.0, 0.0, 0.0],
            order: 2,
        };
        let halfway = AudioEarlyReflectionField::default().lerped(target, 0.5);
        assert_eq!(halfway.count, 1);
        assert!((halfway.taps[0].gain - 0.4).abs() < 1.0e-6);
        assert_eq!(halfway.taps[0].delay_ms, 20.0);
        assert_eq!(halfway.taps[0].order, 2);
    }
