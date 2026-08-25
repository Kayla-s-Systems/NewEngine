use super::*;

impl CameraRuntimeService {
    /// Synchronizes a possessed gameplay camera at render cadence. Character translation
    /// remains fixed-step authoritative, but view rotation and camera spring integration no
    /// longer wait for the next simulation tick. This removes third-person mouse-look jitter.
    pub fn sync_gameplay_camera_now(
        world: &mut World,
        camera: EntityId,
        player: EntityId,
        config: CameraRuntimeServiceConfig,
        dt: f32,
    ) -> bool {
        let Some(controller) = world.get::<FollowTargetCameraController>(camera).copied() else {
            return false;
        };
        if controller.target != player {
            return false;
        }
        let Some((simulation_target_position, simulation_target_body_rotation)) =
            read_entity_world_pose_local_chain(world, player)
        else {
            return false;
        };
        let third_person_mode = !matches!(config.runner, GameplayCameraRunnerKind::FirstPerson);
        let target_position = if third_person_mode {
            config
                .third_person_render_position_ws
                .filter(|position| position.is_finite())
                .unwrap_or(simulation_target_position)
        } else {
            simulation_target_position
        };
        let target_body_rotation = if third_person_mode {
            config
                .third_person_render_rotation_ws
                .filter(|rotation| rotation.is_finite())
                .unwrap_or(simulation_target_body_rotation)
        } else {
            simulation_target_body_rotation
        }
        .normalize_or_identity();
        let player_motor = world.get::<CharacterMotor>(player).copied();
        let player_view_rotation = player_motor
            .map(|motor| Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0))
            .unwrap_or(target_body_rotation)
            .normalize_or_identity();
        if matches!(config.runner, GameplayCameraRunnerKind::FirstPerson) {
            // First-person position is anchored to the stable player root/world-up eye height.
            // Pitch/yaw rotate only the view; they must never orbit the eye point around the body.
            let eye_height = if config.first_person_eye_height.is_finite() {
                config.first_person_eye_height.max(0.01)
            } else {
                controller.offset_ls.y.max(0.01)
            };
            let eye_center = config
                .first_person_anchor_ws
                .filter(|position| position.is_finite())
                .unwrap_or(target_position + Vec3::Y * eye_height);
            let camera_rotation =
                (player_view_rotation * controller.rot_offset).normalize_or_identity();
            let forward_clearance = if config.first_person_forward_clearance.is_finite() {
                config.first_person_forward_clearance.clamp(0.0, 0.20)
            } else {
                0.055
            };
            let camera_position =
                eye_center + (camera_rotation * -Vec3::Z).normalize_or_zero() * forward_clearance;
            let _ = world.insert(
                camera,
                CameraRigComp(CameraRig {
                    position: camera_position,
                    rotation: camera_rotation,
                }),
            );
            let _ = world.insert(camera, FollowTargetCameraMotor::default());
            let _ = world.remove::<GameplayThirdPersonCameraState>(camera);
            write_entity_local_from_world_pose_local_chain(
                world,
                camera,
                camera_position,
                camera_rotation,
            );
            return true;
        }

        // Third-person motion is decomposed into independent translation/orientation state.
        // Free Orbit is stricter than Follow: its pivot must use the exact same current player
        // position as the rendered subject so the character cannot drift away from screen center.
        let orbit_mode = matches!(config.runner, GameplayCameraRunnerKind::ThirdPersonOrbit);
        let mut third_person = world
            .get::<GameplayThirdPersonCameraState>(camera)
            .copied()
            .unwrap_or_default();
        if !third_person.initialized
            || third_person.runner != config.runner
            || third_person.target != player
            || !third_person.anchor_ws.is_finite()
        {
            third_person.runner = config.runner;
            third_person.target = player;
            third_person.anchor_ws = target_position;
            third_person.zoom_z = controller.offset_ls.z;
            if orbit_mode {
                let pivot_offset_ls = if config.third_person_orbit_pivot_offset_ls.is_finite() {
                    config.third_person_orbit_pivot_offset_ls
                } else {
                    Vec3::ZERO
                };
                let pivot_ws = target_position
                    + target_body_rotation.normalize_or_identity() * pivot_offset_ls;
                let rig = world
                    .get::<CameraRigComp>(camera)
                    .copied()
                    .unwrap_or_default()
                    .0;
                let inherited_view = player_motor
                    .map(|motor| (wrap_pi(motor.yaw), motor.pitch.clamp(-1.35, 1.35)))
                    .unwrap_or((0.0, 0.0));
                let (yaw, pitch) =
                    orbit_angles_from_camera(pivot_ws, rig.position).unwrap_or(inherited_view);
                third_person.orbit_yaw = yaw;
                third_person.orbit_pitch = pitch;
                third_person.orbit_pivot_offset_ws =
                    target_body_rotation.normalize_or_identity() * pivot_offset_ls;
            } else {
                third_person.orbit_yaw = player_motor.map(|motor| motor.yaw).unwrap_or(0.0);
                third_person.orbit_pitch = player_motor.map(|motor| motor.pitch).unwrap_or(0.0);
                third_person.orbit_pivot_offset_ws = Vec3::ZERO;
            }
            third_person.collision_distance = 0.0;
            third_person.last_pivot_ws = target_position;
            third_person.last_focus_ws = target_position;
            third_person.last_desired_camera_ws = target_position;
            third_person.last_collision_target_distance = 0.0;
            third_person.initialized = true;
        } else {
            // target_position is already the render-cadence PlayerRenderPose when the engine
            // presentation layer is active. Filtering it again makes camera and rendered avatar
            // follow different trajectories, which shows up as third-person relative jitter.
            // Keep one interpolation owner: presentation publishes the anchor, camera consumes it.
            third_person.anchor_ws = target_position;
            third_person.zoom_z =
                smooth_gameplay_zoom(third_person.zoom_z, controller.offset_ls.z, dt);
        }
        let anchor_ws = third_person.anchor_ws;
        let target_rotation = if orbit_mode {
            Quat::from_euler(
                EulerRot::YXZ,
                third_person.orbit_yaw,
                third_person.orbit_pitch,
                0.0,
            )
            .normalize_or_identity()
        } else {
            player_view_rotation
        };

        let (camera_target_position, focus_position, mut camera_offset) = if orbit_mode {
            // A true orbit has one invariant center: the camera revolves around the same point
            // it looks at. Using the old `(0, height, radius)` offset while looking at another
            // torso point made the avatar drift away from screen center as yaw/pitch changed.
            // Orbit center offset is captured in world space when the mode is entered. Free Orbit
            // must not move its sphere center merely because locomotion rotates the avatar body.
            let pivot = anchor_ws + third_person.orbit_pivot_offset_ws;
            let radius_sign = if third_person.zoom_z < 0.0 { -1.0 } else { 1.0 };
            let radial_offset = Vec3::new(0.0, 0.0, radius_sign * third_person.zoom_z.abs());
            (pivot, pivot, radial_offset)
        } else {
            let mut offset = controller.offset_ls;
            offset.z = third_person.zoom_z;
            let focus = anchor_ws
                + target_body_rotation.normalize_or_identity() * controller.focus_offset_ls;
            (anchor_ws, focus, offset)
        };

        let desired_camera_ws = camera_target_position + target_rotation * camera_offset;
        third_person.last_pivot_ws = camera_target_position;
        third_person.last_focus_ws = focus_position;
        third_person.last_desired_camera_ws = desired_camera_ws;
        let desired_arm_ws = desired_camera_ws - focus_position;
        let desired_arm_distance = desired_arm_ws.length();
        third_person.last_collision_target_distance = desired_arm_distance;
        if desired_arm_distance > 1.0e-5 {
            let desired_arm_ls = target_rotation.inverse() * desired_arm_ws;
            let constrained_arm_ls = {
                let collision_world = world.resource::<CameraSpringArmCollisionWorld>();
                constrain_spring_arm_offset_ls(
                    player,
                    focus_position,
                    target_rotation,
                    desired_arm_ls,
                    CameraSpringArmConfig::default(),
                    collision_world,
                )
            };
            let collision_target_distance = constrained_arm_ls
                .length()
                .clamp(0.001, desired_arm_distance);
            third_person.last_collision_target_distance = collision_target_distance;
            third_person.collision_distance = smooth_collision_release(
                third_person.collision_distance,
                collision_target_distance,
                dt,
            )
            .min(desired_arm_distance);
            let arm_dir_ws = desired_arm_ws / desired_arm_distance;
            let collision_safe_camera_ws =
                focus_position + arm_dir_ws * third_person.collision_distance;
            camera_offset =
                target_rotation.inverse() * (collision_safe_camera_ws - camera_target_position);
        }
        let _ = world.insert(camera, third_person);

        let (next_pos, next_rot) = if orbit_mode {
            // Pure analytic Orbit: exactly one pivot and one radial arm. Generic follow-camera
            // integration is bypassed so there cannot be a second spring writer or angular chase.
            let position = camera_target_position + target_rotation * camera_offset;
            let rotation = orbit_look_at_rotation(position, focus_position);
            (position, rotation)
        } else {
            let rig = world
                .get::<CameraRigComp>(camera)
                .copied()
                .unwrap_or_default();
            let Some(step) = step_follow_camera(
                rig.0.position,
                rig.0.rotation,
                camera_target_position,
                target_rotation,
                focus_position,
                camera_offset,
                controller.rot_offset,
                controller.follow_rotation,
                Vec3::ZERO,
                0.0,
                0.0,
                dt,
            ) else {
                return false;
            };
            (step.next_pos, step.next_rot)
        };
        let _ = world.insert(
            camera,
            CameraRigComp(CameraRig {
                position: next_pos,
                rotation: next_rot,
            }),
        );
        let _ = world.insert(camera, FollowTargetCameraMotor::default());
        write_entity_local_from_world_pose_local_chain(world, camera, next_pos, next_rot);
        true
    }

    #[inline]
    pub fn clear_player_input(world: &mut World, player: EntityId) {
        if let Some(input) = world.get_mut::<MotorInput>(player) {
            *input = MotorInput::default();
        }
    }

    #[inline]
    pub fn report_cursor(world: &World) -> Option<CursorState> {
        world
            .resource::<CameraManagerResource>()
            .map(|manager| manager.last_cursor)
    }
}
