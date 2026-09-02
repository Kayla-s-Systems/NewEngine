#[cfg(test)]
mod skeletal_secondary_motion_tests {
    use super::*;

    #[test]
    fn polyline_sampling_is_topology_agnostic() {
        let points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ];
        let sampled = sample_polyline_normalized(&points, 0.5);
        assert!((sampled.y - 1.5).abs() < 1.0e-5);
    }

    #[test]
    fn exterior_capsule_projection_respects_authored_radius() {
        let mut point = Vec3::new(0.0, 0.0, 0.1);
        project_out_of_secondary_motion_capsule(
            &mut point,
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            0.5,
        );
        assert!((point.length() - 0.5).abs() < 1.0e-5);
    }
}
