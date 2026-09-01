#[cfg(test)]
mod first_person_position_tests {
    use super::*;

    #[test]
    fn first_person_camera_volume_retracts_before_world_geometry() {
        let mut ecs = World::new();
        let player = ecs.spawn();
        let wall = ecs.spawn();
        let mut collision = CameraSpringArmCollisionWorld::default();
        collision.push_aabb(crate::constraints::CameraSpringArmAabbCollider {
            entity: wall,
            min_ws: Vec3::new(-1.0, -1.0, -0.22),
            max_ws: Vec3::new(1.0, 1.0, -0.20),
        });
        let eye = Vec3::ZERO;
        let desired = Vec3::new(0.0, 0.0, -0.30);
        let constrained = constrain_first_person_camera_position(
            player,
            eye,
            desired,
            CameraSpringArmConfig {
                enabled: true,
                probe_radius: 0.055,
                collision_padding: 0.012,
                min_distance: 0.0,
            },
            Some(&collision),
        );
        assert!(
            constrained.z > desired.z,
            "camera must retract before the wall"
        );
        assert!(constrained.z <= 0.0);
    }

    #[test]
    fn first_person_camera_collision_ignores_player_body_proxy() {
        let mut ecs = World::new();
        let player = ecs.spawn();
        let mut collision = CameraSpringArmCollisionWorld::default();
        collision.push(crate::constraints::CameraSpringArmCollider {
            entity: player,
            center_ws: Vec3::new(0.0, 0.0, -0.05),
            radius: 0.45,
        });
        let eye = Vec3::ZERO;
        let desired = Vec3::new(0.0, 0.0, -0.10);
        let constrained = constrain_first_person_camera_position(
            player,
            eye,
            desired,
            CameraSpringArmConfig {
                enabled: true,
                probe_radius: 0.055,
                collision_padding: 0.012,
                min_distance: 0.0,
            },
            Some(&collision),
        );
        assert!((constrained - desired).length() <= 1.0e-6);
    }

    #[test]
    fn ads_camera_position_converges_exactly_to_rendered_rear_sight_anchor() {
        let hip = Vec3::new(0.0, 1.62, -0.045);
        let ads = Vec3::new(-0.013, 1.601, -0.092);
        assert_eq!(first_person_ads_position_contract(hip, Some(ads), 0.0), hip);
        let half = first_person_ads_position_contract(hip, Some(ads), 0.5);
        assert!((half - hip.lerp(ads, 0.5)).length() <= 1.0e-7);
        assert_eq!(first_person_ads_position_contract(hip, Some(ads), 1.0), ads);
        assert_eq!(first_person_ads_position_contract(hip, None, 1.0), hip);
    }

    #[test]
    fn pitch_rotates_view_without_translating_fpp_position() {
        let body = Quat::from_rotation_y(0.37);
        let neutral = body;
        let pitched = (body * Quat::from_rotation_x(1.25)).normalize_or_identity();
        let neutral_contract = first_person_position_contract(body, neutral, 0.045);
        let pitched_contract = first_person_position_contract(body, pitched, 0.045);
        assert!((neutral_contract.0 - pitched_contract.0).length() <= 1.0e-6);
        assert!((neutral_contract.1 - pitched_contract.1).length() <= 1.0e-6);
    }

    #[test]
    fn mouse_yaw_cannot_orbit_camera_around_eye_center() {
        let body = Quat::IDENTITY;
        let forward = first_person_position_contract(body, body, 0.045);
        let right_view = Quat::from_rotation_y(core::f32::consts::FRAC_PI_2);
        let right = first_person_position_contract(body, right_view, 0.045);
        let forward_position = forward.0 + forward.1;
        let right_position = right.0 + right.1;
        // View yaw is orientation-only. The physical eye anchor is body-owned and must not move.
        assert!((right_position - forward_position).length() <= 1.0e-8);
        assert!(right.1.length() <= 1.0e-8);
    }

    #[test]
    fn locomotion_never_moves_or_rotates_first_person_camera() {
        let mut state = GameplayFirstPersonCameraState::default();
        state.initialized = true;
        for aiming in [false, true] {
            for _ in 0..120 {
                let pose = step_first_person_additive_motion(
                    &mut state,
                    FirstPersonPresentationInput {
                        grounded: true,
                        horizontal_speed: 7.5,
                        aiming,
                        ..Default::default()
                    },
                    1.0 / 60.0,
                    18.0,
                    0.42,
                );
                assert!(pose.position_ls.is_finite());
                assert!(pose.position_ls.length() <= 1.0e-8);
                assert!(pose.rotation_ls.dot(Quat::IDENTITY).abs() >= 1.0 - 1.0e-7);
            }
        }
    }

    #[test]
    fn grounded_eye_anchor_rejects_contact_micro_jitter_but_tracks_horizontal_motion() {
        let current = Vec3::new(1.0, 1.62, 2.0);
        let target = Vec3::new(1.15, 1.626, 1.85);
        let next =
            stabilize_first_person_eye_anchor(current, target, true, 1.0 / 60.0, 0.010, 0.060);
        assert!((next.x - target.x).abs() <= 1.0e-8);
        assert!((next.z - target.z).abs() <= 1.0e-8);
        assert!((next.y - current.y).abs() <= 1.0e-8);
    }

    #[test]
    fn grounded_eye_stabilization_consumes_authored_deadband() {
        let current = Vec3::new(0.0, 1.62, 0.0);
        let target = Vec3::new(0.0, 1.626, 0.0);
        let latched =
            stabilize_first_person_eye_anchor(current, target, true, 1.0 / 60.0, 0.010, 0.060);
        let responsive =
            stabilize_first_person_eye_anchor(current, target, true, 1.0 / 60.0, 0.002, 0.060);
        assert!((latched.y - current.y).abs() <= 1.0e-8);
        assert!(responsive.y > current.y);
        assert!(responsive.y < target.y);
    }

    #[test]
    fn meaningful_grounded_eye_height_change_converges_without_teleport() {
        let current = Vec3::new(0.0, 1.62, 0.0);
        let target = Vec3::new(0.0, 1.42, 0.0);
        let next =
            stabilize_first_person_eye_anchor(current, target, true, 1.0 / 60.0, 0.010, 0.060);
        assert!(next.y < current.y);
        assert!(next.y > target.y);
    }

    #[test]
    fn shot_sequence_injects_camera_recoil_once_then_recovers() {
        let mut state = GameplayFirstPersonCameraState {
            initialized: true,
            last_shot_sequence: 10,
            ..Default::default()
        };
        let shot = FirstPersonPresentationInput {
            shot_sequence: 11,
            recoil_pitch_radians: 0.04,
            recoil_pitch_random_radians: 0.0,
            recoil_yaw_radians: 0.0,
            recoil_recovery_hz: 5.0,
            ..Default::default()
        };
        let _ = step_first_person_additive_motion(&mut state, shot, 1.0 / 120.0, 18.0, 0.42);
        let first = state.recoil_pitch_radians;
        let _ = step_first_person_additive_motion(&mut state, shot, 1.0 / 120.0, 18.0, 0.42);
        let second = state.recoil_pitch_radians;
        assert!(first > 0.0);
        assert!(
            second < first,
            "same shot sequence must decay, not inject twice"
        );
        for _ in 0..240 {
            let _ = step_first_person_additive_motion(&mut state, shot, 1.0 / 120.0, 18.0, 0.42);
        }
        assert!(state.recoil_pitch_radians < second * 0.01);
    }

    #[test]
    fn authored_camera_recoil_share_scales_the_same_weapon_impulse() {
        let shot = FirstPersonPresentationInput {
            shot_sequence: 1,
            recoil_pitch_radians: 0.04,
            recoil_recovery_hz: 0.05,
            ..Default::default()
        };
        let mut low = GameplayFirstPersonCameraState {
            initialized: true,
            ..Default::default()
        };
        let mut high = low;
        let _ = step_first_person_additive_motion(&mut low, shot, 1.0 / 1000.0, 18.0, 0.20);
        let _ = step_first_person_additive_motion(&mut high, shot, 1.0 / 1000.0, 18.0, 0.80);
        assert!(high.recoil_pitch_radians > low.recoil_pitch_radians * 3.9);
    }

    #[test]
    fn body_turn_owns_the_base_eye_clearance() {
        let body = Quat::from_rotation_y(0.83);
        let view = Quat::from_rotation_y(-0.41);
        let (base, _) = first_person_position_contract(body, view, 0.045);
        let expected = first_person_horizontal_forward(body) * 0.045;
        assert!((base - expected).length() <= 1.0e-6);
    }
}
