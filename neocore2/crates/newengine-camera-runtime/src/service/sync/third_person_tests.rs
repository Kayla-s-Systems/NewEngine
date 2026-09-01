#[cfg(test)]
mod third_person_response_tests {
    use super::*;
    use newengine_transform::Transform;

    #[test]
    fn catch_up_spring_converges_without_crossing_authored_relative_frame() {
        let target = Vec3::new(0.35, 1.65, 4.5);
        let mut current = Vec3::new(0.0, 1.60, -0.07);
        let mut velocity = Vec3::ZERO;
        let mut previous_error = (target - current).length();
        for _ in 0..240 {
            let (next, next_velocity) =
                step_catch_up_offset(current, velocity, target, 1.0 / 120.0, 2.4, 1.0);
            let error = (target - next).length();
            assert!(error <= previous_error + 1.0e-6);
            assert!((target - current).dot(target - next) >= -1.0e-6);
            current = next;
            velocity = next_velocity;
            previous_error = error;
        }
        assert!((target - current).length() < 0.002);
    }

    #[test]
    fn bounded_look_rotation_never_lags_beyond_authored_fov_fraction_budget() {
        let current = Quat::from_rotation_y(90.0_f32.to_radians());
        let desired = Quat::IDENTITY;
        let max_error = 8.0_f32.to_radians();
        let next = step_bounded_look_rotation(current, desired, 1.0 / 120.0, 14.0, max_error);
        let dot = next.dot(desired).abs().clamp(0.0, 1.0);
        let error = 2.0 * dot.acos();
        assert!(error <= max_error + 1.0e-5);
        assert!(error < 90.0_f32.to_radians());
    }

    #[test]
    fn collision_aware_look_at_blends_between_pre_and_post_collision_composition() {
        let root = Vec3::ZERO;
        let focus = Vec3::new(0.0, 0.95, 0.0);
        let pre = Vec3::new(0.35, 1.65, 4.5);
        let post = focus + (pre - focus) * 0.45;
        let pre_rotation = orbit_look_at_rotation(pre, focus);
        let post_rotation = collision_aware_look_rotation(pre, post, root, focus, 0.45, 1.0);
        let zero_blend = collision_aware_look_rotation(pre, post, root, focus, 0.45, 0.0);
        assert!(zero_blend.dot(pre_rotation).abs() > 0.999999);
        assert!(post_rotation.dot(pre_rotation).abs() < 0.99999);
    }

    #[test]
    fn real_mode_switch_uses_relative_catch_up_instead_of_world_space_player_lag() {
        let mut world = World::new();
        let player = world.spawn();
        let camera = world.spawn();
        let _ = world.insert(player, Transform::default());
        let _ = world.insert(player, CharacterMotor::default());
        let _ = world.insert(camera, Transform::default());
        let _ = world.insert(
            camera,
            GameplayFirstPersonRunner { eye_height: 1.6 }.controller(player),
        );
        let _ = world.insert(camera, CameraRigComp(CameraRig::default()));
        let first_config = CameraRuntimeServiceConfig::default();
        assert!(CameraRuntimeService::sync_gameplay_camera_now(
            &mut world,
            camera,
            player,
            first_config,
            1.0 / 120.0,
        ));
        let source = world
            .get::<CameraRigComp>(camera)
            .copied()
            .unwrap()
            .0
            .position;

        let follow_config = CameraRuntimeServiceConfig {
            runner: GameplayCameraRunnerKind::ThirdPersonFollow,
            ..CameraRuntimeServiceConfig::default()
        };
        let _ = world.insert(
            camera,
            GameplayThirdPersonFollowRunner {
                shoulder_offset: follow_config.third_person_follow_offset_ls,
                focus_offset: follow_config.third_person_follow_focus_offset_ls,
                smooth_time: follow_config.third_person_follow_smooth_time,
                max_speed: follow_config.third_person_follow_max_speed,
            }
            .controller(player),
        );
        assert!(CameraRuntimeService::sync_gameplay_camera_now(
            &mut world,
            camera,
            player,
            follow_config,
            1.0 / 120.0,
        ));
        let state = world
            .get::<GameplayThirdPersonCameraState>(camera)
            .copied()
            .unwrap();
        let first_follow = world
            .get::<CameraRigComp>(camera)
            .copied()
            .unwrap()
            .0
            .position;
        let authored = follow_config.third_person_follow_offset_ls;
        assert!(state.catch_up_active);
        assert!((first_follow - source).length() < (authored - source).length());
        assert!(
            (first_follow - authored).length()
                > follow_config.third_person_catch_up_settle_distance_m
        );

        // Moving the player root translates the complete catch-up frame exactly; only the local
        // relative offset continues springing. There is no second world-space lag owner.
        let _ = world.insert(
            player,
            Transform {
                position: Vec3::new(5.0, 0.0, 0.0),
                ..Transform::default()
            },
        );
        assert!(CameraRuntimeService::sync_gameplay_camera_now(
            &mut world,
            camera,
            player,
            follow_config,
            1.0 / 120.0,
        ));
        let moved = world
            .get::<CameraRigComp>(camera)
            .copied()
            .unwrap()
            .0
            .position;
        let moved_state = world
            .get::<GameplayThirdPersonCameraState>(camera)
            .copied()
            .unwrap();
        let current_player_ws = Vec3::new(5.0, 0.0, 0.0);
        assert!(
            ((moved - current_player_ws) - moved_state.catch_up_offset_ls).length() < 1.0e-5,
            "world translation must remain exact while only the relative camera offset catches up"
        );
    }
}
