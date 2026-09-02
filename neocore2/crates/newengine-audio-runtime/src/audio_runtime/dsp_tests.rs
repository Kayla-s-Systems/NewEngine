use super::*;

    fn snapshot(emitter: [f32; 3]) -> SpatialMixSnapshot {
        SpatialMixSnapshot {
            emitter_position: emitter,
            left_ear: [-0.1, 0.0, 0.0],
            right_ear: [0.1, 0.0, 0.0],
        }
    }

    #[test]
    fn direct_pan_does_not_apply_a_second_distance_law() {
        let near = direct_stereo_gains(snapshot([0.0, 0.0, 1.0]));
        let far = direct_stereo_gains(snapshot([0.0, 0.0, 100.0]));
        assert!((near[0] - far[0]).abs() < 1.0e-6);
        assert!((near[1] - far[1]).abs() < 1.0e-6);
        assert!((near[0] - 1.0).abs() < 1.0e-6);
        assert!((near[1] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn direct_pan_preserves_directionality_without_distance_attenuation() {
        let right = direct_stereo_gains(snapshot([10.0, 0.0, 2.0]));
        let right_far = direct_stereo_gains(snapshot([100.0, 0.0, 20.0]));
        assert!(right[1] > right[0]);
        assert!((right[0] - right_far[0]).abs() < 1.0e-6);
        assert!((right[1] - right_far[1]).abs() < 1.0e-6);
    }


#[test]
fn reverb_send_pair_has_one_indirect_energy_budget() {
    let gains = normalized_reverb_send_gains(0.8, 0.8);
    assert!((gains[0] - 0.5).abs() < 1.0e-6, "gains={gains:?}");
    assert!((gains[1] - 0.5).abs() < 1.0e-6, "gains={gains:?}");
    assert!((gains[0] + gains[1] - 1.0).abs() < 1.0e-6);

    let single = normalized_reverb_send_gains(0.35, 0.0);
    assert!((single[0] - 0.35).abs() < 1.0e-6);
    assert_eq!(single[1], 0.0);
}

#[test]
fn explicit_early_reflection_taps_are_individually_bounded_without_collapsing_timing() {
    let sample_rate = SampleRate::new(48_000).expect("sample rate");
    let channels = ChannelCount::new(1).expect("mono");
    let mut field = AudioEarlyReflectionField::empty();
    field.count = 2;
    field.taps[0] = AudioEarlyReflectionTap {
        delay_ms: 5.0,
        gain: 2.0,
        high_frequency_gain: 1.0,
        direction: [1.0, 0.0, 0.0],
        order: 1,
    };
    field.taps[1] = AudioEarlyReflectionTap {
        delay_ms: 10.0,
        gain: 0.8,
        high_frequency_gain: 1.0,
        direction: [-1.0, 0.0, 0.0],
        order: 1,
    };
    let params = ReverbSendSnapshot::from_send(AudioReverbSend {
        room_bus_id: 0,
        gain: 2.0,
        preset: AudioReverbPreset {
            early_reflections_gain: 2.0,
            ..AudioReverbPreset::room()
        },
        early_reflections: field,
        early_reflection_direction: [0.0; 3],
    });
    assert_eq!(params.gain, 1.0);
    assert_eq!(params.early_reflections_gain, 1.0);

    let mut tank = ReverbTank::new_early_only(sample_rate, channels);
    let rendered = (0..520usize)
        .map(|frame| tank.process(if frame == 0 { 1.0 } else { 0.0 }, params))
        .collect::<Vec<_>>();
    assert!((rendered[240] - 1.0).abs() < 1.0e-5, "first={}", rendered[240]);
    assert!((rendered[480] - 0.8).abs() < 1.0e-5, "second={}", rendered[480]);
}
