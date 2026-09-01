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
