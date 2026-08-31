use super::*;
use newengine_transform::Transform;

#[test]
fn gameplay_zoom_normalizes_line_and_legacy_wheel_packets() {
    assert!((normalized_gameplay_zoom_steps(1.0) - 1.0).abs() <= f32::EPSILON);
    assert!((normalized_gameplay_zoom_steps(120.0) - 1.0).abs() <= f32::EPSILON);
    assert!((normalized_gameplay_zoom_steps(-240.0) + 2.0).abs() <= f32::EPSILON);
    assert_eq!(normalized_gameplay_zoom_steps(f32::NAN), 0.0);
}

#[test]
fn third_person_mouse_wheel_zoom_is_bounded() {
    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        ..CameraRuntimeServiceConfig::default()
    };

    let initial = world
        .get::<FollowTargetCameraController>(camera)
        .unwrap()
        .offset_ls
        .z;
    assert!(CameraRuntimeService::apply_gameplay_camera_zoom(
        &mut world, camera, config, 1.0,
    ));
    let zoomed_in = world
        .get::<FollowTargetCameraController>(camera)
        .unwrap()
        .offset_ls
        .z;
    assert!(zoomed_in < initial);

    for _ in 0..64 {
        let _ = CameraRuntimeService::apply_gameplay_camera_zoom(&mut world, camera, config, 4.0);
    }
    let min_zoom = world
        .get::<FollowTargetCameraController>(camera)
        .unwrap()
        .offset_ls
        .z;
    assert!((min_zoom - 1.35).abs() < 1.0e-4);

    for _ in 0..64 {
        let _ = CameraRuntimeService::apply_gameplay_camera_zoom(&mut world, camera, config, -4.0);
    }
    let max_zoom = world
        .get::<FollowTargetCameraController>(camera)
        .unwrap()
        .offset_ls
        .z;
    assert!((max_zoom - 10.0).abs() < 1.0e-4);
}

#[test]
fn third_person_orbit_look_does_not_mutate_character_view_or_body_intent() {
    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let motor = CharacterMotor {
        yaw: 0.35,
        pitch: -0.12,
        ..CharacterMotor::default()
    };
    let player_transform = Transform {
        position: Vec3::new(1.0, 2.0, 3.0),
        rotation: Quat::from_rotation_y(0.7),
        scale: Vec3::ONE,
    };
    let _ = world.insert(player, player_transform);
    let _ = world.insert(player, motor);
    let _ = world.insert(player, MotorInput::default());
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let _ = world.insert(
        camera,
        CameraRigComp(CameraRig {
            position: Vec3::new(1.0, 2.0, 7.8),
            rotation: Quat::IDENTITY,
        }),
    );
    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        ..CameraRuntimeServiceConfig::default()
    };

    assert!(CameraRuntimeService::apply_gameplay_camera_orbit_look(
        &mut world,
        camera,
        player,
        config,
        Vec2::new(80.0, -25.0),
        true,
    ));
    CameraRuntimeService::apply_player_input(
        &mut world,
        player,
        0,
        Vec2::new(80.0, -25.0),
        true,
        2.0,
        GameplayCameraRunnerKind::ThirdPersonOrbit,
        65.0_f32.to_radians(),
        75.0_f32.to_radians(),
        false,
    );

    let after = world.get::<CharacterMotor>(player).copied().unwrap();
    assert_eq!(after.yaw, motor.yaw);
    assert_eq!(after.pitch, motor.pitch);
    let after_transform = world.get::<Transform>(player).copied().unwrap();
    assert!((after_transform.position - player_transform.position).length() < 1.0e-6);
    assert!(
        after_transform
            .rotation
            .dot(player_transform.rotation)
            .abs()
            > 0.999999
    );
    assert!((after_transform.scale - player_transform.scale).length() < 1.0e-6);
    let player_input = world.get::<MotorInput>(player).copied().unwrap();
    assert!(!player_input.look_active);
    assert_eq!(player_input.look_delta, Vec2::ZERO);

    let camera_state = world
        .get::<GameplayThirdPersonCameraState>(camera)
        .copied()
        .unwrap();
    assert!(camera_state.orbit_yaw.is_finite());
    assert!(camera_state.orbit_pitch.is_finite());
}

#[test]
fn first_person_look_clamps_downward_pitch_before_inner_torso_can_enter_view() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(
        player,
        CharacterMotor {
            pitch: 0.0,
            look_sens: 1.0,
            pitch_limit: 1.54,
            ..CharacterMotor::default()
        },
    );
    let _ = world.insert(player, MotorInput::default());

    CameraRuntimeService::apply_player_input(
        &mut world,
        player,
        0,
        Vec2::new(0.0, -10.0),
        true,
        1.0,
        GameplayCameraRunnerKind::FirstPerson,
        65.0_f32.to_radians(),
        75.0_f32.to_radians(),
        true,
    );

    let motor = world.get::<CharacterMotor>(player).copied().unwrap();
    assert!(motor.pitch >= -75.0_f32.to_radians() - 1.0e-6);
    assert!(motor.pitch <= 0.0);
}

#[test]
fn first_person_yaw_remains_free_while_body_is_stationary() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(
        player,
        CharacterMotor {
            yaw: 0.0,
            look_sens: 1.0,
            ..CharacterMotor::default()
        },
    );
    let _ = world.insert(player, MotorInput::default());

    let apply_look = |world: &mut World, dx: f32| {
        CameraRuntimeService::apply_player_input(
            world,
            player,
            0,
            Vec2::new(dx, 0.0),
            true,
            1.0,
            GameplayCameraRunnerKind::FirstPerson,
            65.0_f32.to_radians(),
            85.0_f32.to_radians(),
            true,
        );
    };

    apply_look(&mut world, 2.0);
    let first_yaw = world.get::<CharacterMotor>(player).copied().unwrap().yaw;
    assert!((wrap_pi(first_yaw - 2.0)).abs() <= 1.0e-5);

    // The old FPP path stopped here at the body-relative yaw limit. The camera must continue
    // rotating even though the physical body has not advanced yet.
    apply_look(&mut world, 0.5);
    let continued_yaw = world.get::<CharacterMotor>(player).copied().unwrap().yaw;
    assert!((wrap_pi(continued_yaw - 2.5)).abs() <= 1.0e-5);

    let body = world.get::<Transform>(player).copied().unwrap();
    let (body_yaw, body_pitch, _) = body.rotation.to_euler(EulerRot::YXZ);
    assert!(body_yaw.abs() <= 1.0e-6);
    assert!(body_pitch.abs() <= 1.0e-6);
}

#[test]
fn first_person_yaw_crosses_previous_body_limit_without_snap_or_capture() {
    let mut world = World::new();
    let player = world.spawn();
    let _ = world.insert(player, Transform::default());
    let initial_yaw = 150.0_f32.to_radians();
    let _ = world.insert(
        player,
        CharacterMotor {
            yaw: initial_yaw,
            look_sens: 1.0,
            ..CharacterMotor::default()
        },
    );
    let _ = world.insert(player, MotorInput::default());

    CameraRuntimeService::apply_player_input(
        &mut world,
        player,
        0,
        Vec2::new(0.1, 0.0),
        true,
        1.0,
        GameplayCameraRunnerKind::FirstPerson,
        65.0_f32.to_radians(),
        85.0_f32.to_radians(),
        true,
    );
    let advanced = world.get::<CharacterMotor>(player).copied().unwrap().yaw;
    assert!((wrap_pi(advanced - wrap_pi(initial_yaw + 0.1))).abs() <= 1.0e-5);

    CameraRuntimeService::apply_player_input(
        &mut world,
        player,
        0,
        Vec2::new(-0.2, 0.0),
        true,
        1.0,
        GameplayCameraRunnerKind::FirstPerson,
        65.0_f32.to_radians(),
        85.0_f32.to_radians(),
        true,
    );
    let reversed = world.get::<CharacterMotor>(player).copied().unwrap().yaw;
    assert!((wrap_pi(reversed - wrap_pi(initial_yaw - 0.1))).abs() <= 1.0e-5);
}

#[test]
fn third_person_orbit_keeps_visual_center_on_camera_forward_axis() {
    use newengine_transform::Transform;

    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let player_position = Vec3::new(2.0, 0.0, -1.0);
    let _ = world.insert(
        player,
        Transform {
            position: player_position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    let _ = world.insert(player, CharacterMotor::default());
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let _ = world.insert(camera, CameraRigComp(CameraRig::default()));
    let _ = world.insert(camera, FollowTargetCameraMotor::default());
    let _ = world.insert(camera, Transform::default());

    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        third_person_orbit_pivot_offset_ls: Vec3::new(0.15, 0.20, -0.10),
        ..CameraRuntimeServiceConfig::default()
    };
    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 60.0,
    ));

    let rig = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    let pivot = player_position + config.third_person_orbit_pivot_offset_ls;
    let to_pivot = (pivot - rig.position).normalize_or_zero();
    let camera_forward = (rig.rotation * Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    assert!(camera_forward.dot(to_pivot) > 0.9999);
    assert!(((rig.position - pivot).length() - 4.8).abs() < 1.0e-4);

    // Orbit must stay centered while the player translates as well; no follow-lag is allowed
    // to move the subject off the optical axis.
    let moved_player_position = Vec3::new(5.0, 0.0, 2.0);
    if let Some(transform) = world.get_mut::<Transform>(player) {
        transform.position = moved_player_position;
    }
    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 60.0,
    ));
    let moved_rig = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    let moved_pivot = moved_player_position + config.third_person_orbit_pivot_offset_ls;
    let moved_to_pivot = (moved_pivot - moved_rig.position).normalize_or_zero();
    let moved_forward = (moved_rig.rotation * Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    assert!(moved_forward.dot(moved_to_pivot) > 0.9999);
    assert!(((moved_rig.position - moved_pivot).length() - 4.8).abs() < 1.0e-4);
}

#[test]
fn third_person_sync_applies_spring_arm_collision_without_mutating_player() {
    use newengine_transform::Transform;

    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let wall = world.spawn();
    let motor = CharacterMotor {
        yaw: 0.0,
        pitch: 0.0,
        ..CharacterMotor::default()
    };
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(player, motor);
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let _ = world.insert(camera, CameraRigComp(CameraRig::default()));
    let _ = world.insert(camera, FollowTargetCameraMotor::default());
    let _ = world.insert(camera, Transform::default());

    let mut collision_world = CameraSpringArmCollisionWorld::default();
    collision_world.push_aabb(crate::constraints::CameraSpringArmAabbCollider {
        entity: wall,
        min_ws: Vec3::new(-2.0, 0.0, 2.0),
        max_ws: Vec3::new(2.0, 3.0, 2.2),
    });
    world.insert_resource(collision_world);

    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        ..CameraRuntimeServiceConfig::default()
    };
    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 60.0,
    ));

    let rig = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    assert!(rig.position.z > 0.75);
    assert!(rig.position.z < 2.0);
    let pivot = config.third_person_orbit_pivot_offset_ls;
    let to_pivot = (pivot - rig.position).normalize_or_zero();
    let camera_forward = (rig.rotation * Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    assert!(camera_forward.dot(to_pivot) > 0.9999);
    let after = world.get::<CharacterMotor>(player).copied().unwrap();
    assert_eq!(after.yaw, motor.yaw);
    assert_eq!(after.pitch, motor.pitch);
}

#[test]
fn spring_arm_collision_retracts_immediately_and_releases_smoothly() {
    let blocked = smooth_collision_release(4.0, 1.5, 1.0 / 60.0);
    assert!((blocked - 1.5).abs() <= f32::EPSILON);
    let seam_noise = smooth_collision_release(1.5, 1.494, 1.0 / 144.0);
    assert!((seam_noise - 1.5).abs() <= f32::EPSILON);
    let released = smooth_collision_release(1.5, 4.0, 1.0 / 60.0);
    assert!(released > 1.5);
    assert!(released < 4.0);
}

#[test]
fn third_person_follow_consumes_render_pose_without_second_anchor_filter() {
    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(player, CharacterMotor::default());
    let _ = world.insert(
        camera,
        GameplayThirdPersonFollowRunner::default().controller(player),
    );
    let _ = world.insert(camera, CameraRigComp(CameraRig::default()));
    let _ = world.insert(camera, Transform::default());
    let mut config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonFollow,
        third_person_render_position_ws: Some(Vec3::new(4.0, 0.0, 0.0)),
        third_person_render_rotation_ws: Some(Quat::IDENTITY),
        ..CameraRuntimeServiceConfig::default()
    };

    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 144.0,
    ));
    let first = world.get::<CameraRigComp>(camera).copied().unwrap().0;

    config.third_person_render_position_ws = Some(Vec3::new(4.5, 0.0, 0.0));
    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 30.0,
    ));
    let second = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    assert!(((second.position - first.position) - Vec3::new(0.5, 0.0, 0.0)).length() < 1.0e-5);
}

#[test]
fn orbit_look_before_sync_inherits_view_at_vertical_pole() {
    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let yaw = -1.05;
    let pitch = 0.22;
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(
        player,
        CharacterMotor {
            yaw,
            pitch,
            ..CharacterMotor::default()
        },
    );
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let _ = world.insert(
        camera,
        CameraRigComp(CameraRig {
            position: Vec3::new(-1.0e-6, 1.6, 1.0e-6),
            rotation: Quat::IDENTITY,
        }),
    );
    let _ = world.insert(camera, Transform::default());
    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        ..CameraRuntimeServiceConfig::default()
    };

    assert!(CameraRuntimeService::apply_gameplay_camera_orbit_look(
        &mut world,
        camera,
        player,
        config,
        Vec2::new(1.0, 0.0),
        true,
    ));
    let state = world
        .get::<GameplayThirdPersonCameraState>(camera)
        .copied()
        .unwrap();
    let expected_yaw = wrap_pi(yaw + 0.0028);
    assert!((wrap_pi(state.orbit_yaw - expected_yaw)).abs() < 1.0e-5);
    assert!((state.orbit_pitch - pitch).abs() < 1.0e-5);

    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 144.0,
    ));
    let after_sync = world
        .get::<GameplayThirdPersonCameraState>(camera)
        .copied()
        .unwrap();
    assert!((wrap_pi(after_sync.orbit_yaw - expected_yaw)).abs() < 1.0e-5);
    assert!((after_sync.orbit_pitch - pitch).abs() < 1.0e-5);
}

#[test]
fn first_person_to_orbit_inherits_view_at_vertical_pole() {
    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let yaw = 1.15;
    let pitch = 0.28;
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(
        player,
        CharacterMotor {
            yaw,
            pitch,
            ..CharacterMotor::default()
        },
    );
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    // Mimic the first-person eye: almost exactly above the orbit pivot. Tiny X/Z noise
    // must not decide which side of the player the orbit camera appears on.
    let _ = world.insert(
        camera,
        CameraRigComp(CameraRig {
            position: Vec3::new(1.0e-6, 1.6, -1.0e-6),
            rotation: Quat::IDENTITY,
        }),
    );
    let _ = world.insert(camera, Transform::default());
    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        ..CameraRuntimeServiceConfig::default()
    };

    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 144.0,
    ));
    let state = world
        .get::<GameplayThirdPersonCameraState>(camera)
        .copied()
        .unwrap();
    assert!((wrap_pi(state.orbit_yaw - yaw)).abs() < 1.0e-5);
    assert!((state.orbit_pitch - pitch).abs() < 1.0e-5);
    let rig = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    let expected_dir = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::Z;
    let actual_dir = rig.position.normalize_or_zero();
    assert!(actual_dir.dot(expected_dir.normalize_or_zero()) > 0.99999);
}
#[test]
fn middle_mouse_drag_zoom_changes_orbit_radius_and_is_bounded() {
    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        ..CameraRuntimeServiceConfig::default()
    };

    let initial = world
        .get::<FollowTargetCameraController>(camera)
        .unwrap()
        .offset_ls
        .z;
    assert!(CameraRuntimeService::apply_gameplay_camera_drag_zoom(
        &mut world, camera, config, -30.0,
    ));
    let zoomed_in = world
        .get::<FollowTargetCameraController>(camera)
        .unwrap()
        .offset_ls
        .z;
    assert!(zoomed_in < initial);

    assert!(CameraRuntimeService::apply_gameplay_camera_drag_zoom(
        &mut world, camera, config, 60.0,
    ));
    let zoomed_out = world
        .get::<FollowTargetCameraController>(camera)
        .unwrap()
        .offset_ls
        .z;
    assert!(zoomed_out > zoomed_in);

    for _ in 0..64 {
        let _ = CameraRuntimeService::apply_gameplay_camera_drag_zoom(
            &mut world, camera, config, -240.0,
        );
    }
    let min_zoom = world
        .get::<FollowTargetCameraController>(camera)
        .unwrap()
        .offset_ls
        .z;
    assert!((min_zoom - 1.35).abs() < 1.0e-4);

    for _ in 0..64 {
        let _ = CameraRuntimeService::apply_gameplay_camera_drag_zoom(
            &mut world, camera, config, 240.0,
        );
    }
    let max_zoom = world
        .get::<FollowTargetCameraController>(camera)
        .unwrap()
        .offset_ls
        .z;
    assert!((max_zoom - 10.0).abs() < 1.0e-4);
}
#[test]
fn active_orbit_freezes_pivot_alignment_until_mode_reentry() {
    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(player, CharacterMotor::default());
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let _ = world.insert(
        camera,
        CameraRigComp(CameraRig {
            position: Vec3::new(0.0, 1.0, 4.8),
            rotation: Quat::IDENTITY,
        }),
    );
    let _ = world.insert(camera, Transform::default());
    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        third_person_orbit_pivot_offset_ls: Vec3::new(0.2, 0.3, -0.1),
        ..CameraRuntimeServiceConfig::default()
    };
    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 144.0,
    ));
    let first = world.get::<CameraRigComp>(camera).copied().unwrap().0;

    let changed_config = CameraRuntimeServiceConfig {
        third_person_orbit_pivot_offset_ls: Vec3::new(-0.8, 1.1, 0.7),
        ..config
    };
    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        changed_config,
        1.0 / 144.0,
    ));
    let second = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    assert!((second.position - first.position).length() < 1.0e-6);
    assert!(second.rotation.dot(first.rotation).abs() > 0.999999);
}

#[test]
fn active_orbit_pivot_is_independent_from_character_body_facing() {
    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(player, CharacterMotor::default());
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let _ = world.insert(
        camera,
        CameraRigComp(CameraRig {
            position: Vec3::new(0.0, 1.0, 4.8),
            rotation: Quat::IDENTITY,
        }),
    );
    let _ = world.insert(camera, Transform::default());
    let mut config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        third_person_orbit_pivot_offset_ls: Vec3::new(0.2, 0.4, -0.15),
        third_person_render_rotation_ws: Some(Quat::IDENTITY),
        ..CameraRuntimeServiceConfig::default()
    };
    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 144.0,
    ));
    let first = world.get::<CameraRigComp>(camera).copied().unwrap().0;

    config.third_person_render_rotation_ws = Some(Quat::from_rotation_y(2.2));
    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 144.0,
    ));
    let second = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    assert!((second.position - first.position).length() < 1.0e-6);
    assert!(second.rotation.dot(first.rotation).abs() > 0.999999);
}

#[test]
fn third_person_orbit_repeated_sync_is_pose_stable_without_input() {
    use newengine_transform::Transform;

    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let _ = world.insert(player, Transform::default());
    let _ = world.insert(
        player,
        CharacterMotor {
            yaw: 1.1,
            pitch: 0.4,
            ..CharacterMotor::default()
        },
    );
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let _ = world.insert(
        camera,
        CameraRigComp(CameraRig {
            position: Vec3::new(0.0, 0.0, 4.8),
            rotation: Quat::IDENTITY,
        }),
    );
    let _ = world.insert(camera, Transform::default());
    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        ..CameraRuntimeServiceConfig::default()
    };

    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 60.0,
    ));
    let first = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    if let Some(motor) = world.get_mut::<CharacterMotor>(player) {
        motor.yaw = -2.2;
        motor.pitch = -0.7;
    }
    for _ in 0..120 {
        assert!(CameraRuntimeService::sync_gameplay_camera_now(
            &mut world,
            camera,
            player,
            config,
            1.0 / 144.0,
        ));
    }
    let last = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    assert!((last.position - first.position).length() < 1.0e-6);
    assert!(last.rotation.dot(first.rotation).abs() > 0.999999);
    let motor = world.get::<CharacterMotor>(player).copied().unwrap();
    assert!((motor.yaw + 2.2).abs() < 1.0e-6);
    assert!((motor.pitch + 0.7).abs() < 1.0e-6);
}
#[test]
fn third_person_orbit_centers_on_render_pose_override_not_fixed_pose() {
    use newengine_transform::Transform;

    let mut world = World::new();
    let player = world.spawn();
    let camera = world.spawn();
    let simulation_position = Vec3::new(10.0, 0.0, 0.0);
    let render_position = Vec3::new(4.0, 0.0, 0.0);
    let _ = world.insert(
        player,
        Transform {
            position: simulation_position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    let _ = world.insert(player, CharacterMotor::default());
    let _ = world.insert(
        camera,
        GameplayThirdPersonOrbitRunner::default().controller(player),
    );
    let _ = world.insert(
        camera,
        CameraRigComp(CameraRig {
            position: render_position + Vec3::new(0.0, 0.0, 4.8),
            rotation: Quat::IDENTITY,
        }),
    );
    let _ = world.insert(camera, Transform::default());
    let config = CameraRuntimeServiceConfig {
        runner: GameplayCameraRunnerKind::ThirdPersonOrbit,
        third_person_render_position_ws: Some(render_position),
        third_person_render_rotation_ws: Some(Quat::IDENTITY),
        ..CameraRuntimeServiceConfig::default()
    };

    assert!(CameraRuntimeService::sync_gameplay_camera_now(
        &mut world,
        camera,
        player,
        config,
        1.0 / 144.0,
    ));
    let rig = world.get::<CameraRigComp>(camera).copied().unwrap().0;
    let to_render_target = (render_position - rig.position).normalize_or_zero();
    let forward = (rig.rotation * -Vec3::Z).normalize_or_zero();
    assert!(forward.dot(to_render_target) > 0.9999);
    assert!(((rig.position - render_position).length() - 4.8).abs() < 1.0e-4);
    assert!((rig.position.x - simulation_position.x).abs() > 5.0);
}
