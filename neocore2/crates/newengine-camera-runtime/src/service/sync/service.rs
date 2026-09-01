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
            config
                .first_person_body_rotation_ws
                .filter(|rotation| rotation.is_finite())
                .unwrap_or(simulation_target_body_rotation)
        }
        .normalize_or_identity();
        let player_motor = world.get::<CharacterMotor>(player).copied();
        let player_view_rotation = player_motor
            .map(|motor| Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0))
            .unwrap_or(target_body_rotation)
            .normalize_or_identity();
        let runner_history = world
            .get::<GameplayCameraRunnerHistory>(camera)
            .copied()
            .unwrap_or_default();
        let entering_from_other_runner = runner_history.initialized
            && runner_history.target == player
            && runner_history.runner != config.runner;
        if matches!(config.runner, GameplayCameraRunnerKind::FirstPerson) {
            // First-person position is anchored to the stable player root/world-up eye height.
            // Pitch/yaw rotate only the view; they must never orbit the eye point around the body.
            let eye_height = if config.first_person_eye_height.is_finite() {
                config.first_person_eye_height.max(0.01)
            } else {
                controller.offset_ls.y.max(0.01)
            };
            let raw_eye_center = config
                .first_person_anchor_ws
                .filter(|position| position.is_finite())
                .unwrap_or(target_position + Vec3::Y * eye_height);
            let camera_rotation =
                (player_view_rotation * controller.rot_offset).normalize_or_identity();
            let forward_clearance = if config.first_person_forward_clearance.is_finite() {
                config.first_person_forward_clearance.clamp(0.0, 0.08)
            } else {
                0.045
            };
            let (base_offset, _) = first_person_position_contract(
                target_body_rotation,
                camera_rotation,
                forward_clearance,
            );
            let mut first_person = world
                .get::<GameplayFirstPersonCameraState>(camera)
                .copied()
                .unwrap_or_default();
            if !first_person.initialized
                || first_person.target != player
                || !first_person.stable_eye_anchor_ws.is_finite()
            {
                first_person.target = player;
                first_person.stable_eye_anchor_ws = raw_eye_center;
                first_person.aim_alpha = if config.first_person_presentation.aiming {
                    1.0
                } else {
                    0.0
                };
                first_person.recoil_pitch_radians = 0.0;
                first_person.recoil_yaw_radians = 0.0;
                first_person.last_shot_sequence = config.first_person_presentation.shot_sequence;
                first_person.initialized = true;
            } else {
                first_person.stable_eye_anchor_ws = stabilize_first_person_eye_anchor(
                    first_person.stable_eye_anchor_ws,
                    raw_eye_center,
                    config.first_person_presentation.grounded,
                    dt,
                    config.first_person_grounded_eye_deadband_m,
                    config.first_person_grounded_eye_time_constant_seconds,
                );
            }
            let eye_center = first_person.stable_eye_anchor_ws;
            let additive = step_first_person_additive_motion(
                &mut first_person,
                config.first_person_presentation,
                dt,
                config.first_person_aim_response_hz,
                config.first_person_camera_recoil_share,
            );
            let hip_camera_position = eye_center + base_offset;
            let authored_camera_position = first_person_ads_position_contract(
                hip_camera_position,
                config.first_person_ads_anchor_ws,
                first_person.aim_alpha,
            );
            let desired_camera_position =
                authored_camera_position + camera_rotation * additive.position_ls;

            // Do not self-collide the camera against the local owner's head/neck envelope. In a
            // full-body FPP contract those camera-near surfaces are removed from owner rendering.
            // Projecting an eye-position camera outside a synthetic head sphere pushed the camera
            // 10-15 cm in front of the face and was the direct cause of the external/headless-body
            // viewpoint and contact-dependent pops. World collision remains authoritative.
            let camera_position = {
                let collision_world = world.resource::<CameraSpringArmCollisionWorld>();
                constrain_first_person_camera_position(
                    player,
                    eye_center,
                    desired_camera_position,
                    CameraSpringArmConfig {
                        enabled: config.first_person_collision_enabled,
                        probe_radius: config.first_person_collision_probe_radius,
                        collision_padding: config.first_person_collision_padding,
                        min_distance: 0.0,
                    },
                    collision_world,
                )
            };
            let rendered_camera_rotation =
                (camera_rotation * additive.rotation_ls).normalize_or_identity();
            let _ = world.insert(camera, first_person);
            let _ = world.insert(
                camera,
                CameraRigComp(CameraRig {
                    position: camera_position,
                    rotation: rendered_camera_rotation,
                }),
            );
            let _ = world.insert(camera, FollowTargetCameraMotor::default());
            let _ = world.remove::<GameplayThirdPersonCameraState>(camera);
            let _ = world.insert(
                camera,
                GameplayCameraRunnerHistory {
                    target: player,
                    runner: config.runner,
                    initialized: true,
                },
            );
            write_entity_local_from_world_pose_local_chain(
                world,
                camera,
                camera_position,
                rendered_camera_rotation,
            );
            return true;
        }

        let _ = world.remove::<GameplayFirstPersonCameraState>(camera);

        // Third-person motion is decomposed into independent translation/orientation state.
        // Free Orbit is stricter than Follow: its pivot must use the exact same current player
        // position as the rendered subject so the character cannot drift away from screen center.
        let orbit_mode = matches!(config.runner, GameplayCameraRunnerKind::ThirdPersonOrbit);
        let entry_rig = world
            .get::<CameraRigComp>(camera)
            .copied()
            .unwrap_or_default()
            .0;
        let mut third_person = world
            .get::<GameplayThirdPersonCameraState>(camera)
            .copied()
            .unwrap_or_default();
        let state_reinitialized = !third_person.initialized
            || third_person.runner != config.runner
            || third_person.target != player
            || !third_person.anchor_ws.is_finite();
        if state_reinitialized {
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
                let inherited_view = player_motor
                    .map(|motor| {
                        (
                            wrap_pi(motor.yaw),
                            motor.pitch.clamp(
                                config.third_person_orbit_pitch_min_radians,
                                config.third_person_orbit_pitch_max_radians,
                            ),
                        )
                    })
                    .unwrap_or((0.0, 0.0));
                let (yaw, pitch) = orbit_angles_from_camera(
                    pivot_ws,
                    entry_rig.position,
                    config.third_person_orbit_pitch_min_radians,
                    config.third_person_orbit_pitch_max_radians,
                )
                .unwrap_or(inherited_view);
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
            third_person.collision_velocity = 0.0;
            third_person.collision_blocked = false;
            third_person.catch_up_offset_ls = Vec3::ZERO;
            third_person.catch_up_velocity_ls = Vec3::ZERO;
            third_person.catch_up_active = false;
            third_person.look_rotation = entry_rig.rotation.normalize_or_identity();
            third_person.look_initialized =
                entering_from_other_runner && entry_rig.rotation.is_finite();
            third_person.last_pivot_ws = target_position;
            third_person.last_focus_ws = target_position;
            third_person.last_desired_camera_ws = target_position;
            third_person.last_collision_target_distance = 0.0;
            third_person.initialized = true;
        } else {
            // target_position is already the render-cadence PlayerRenderPose when the engine
            // presentation layer is active. Catch-up operates only in orbit-relative camera space;
            // target translation therefore remains exact and cannot introduce avatar/camera drift.
            third_person.anchor_ws = target_position;
            third_person.zoom_z = smooth_gameplay_zoom(
                third_person.zoom_z,
                controller.offset_ls.z,
                dt,
                config.zoom_smooth_time_seconds,
            );
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

        let (camera_target_position, focus_position, authored_camera_offset) = if orbit_mode {
            // A true orbit has one invariant center: the camera revolves around the same point
            // it looks at. Orbit center offset is captured in world space when the mode is entered.
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

        // Catch up from the already-rendered source frame in camera-local/orbit-relative space.
        // This is activated only for an actual gameplay runner transition. Fresh camera startup
        // cuts directly to the authored frame and therefore does not invent a hidden intro blend.
        if state_reinitialized {
            third_person.catch_up_offset_ls = authored_camera_offset;
            third_person.catch_up_velocity_ls = Vec3::ZERO;
            third_person.catch_up_active = false;
            if entering_from_other_runner && config.third_person_catch_up_enabled {
                let source_offset_ls =
                    target_rotation.inverse() * (entry_rig.position - camera_target_position);
                let catch_up_distance = (source_offset_ls - authored_camera_offset).length();
                let max_distance = config
                    .third_person_catch_up_max_distance_m
                    .clamp(0.01, 100.0);
                let settle_distance = config
                    .third_person_catch_up_settle_distance_m
                    .clamp(0.0, max_distance);
                if source_offset_ls.is_finite()
                    && catch_up_distance.is_finite()
                    && catch_up_distance <= max_distance
                    && catch_up_distance > settle_distance
                {
                    third_person.catch_up_offset_ls = source_offset_ls;
                    third_person.catch_up_active = true;
                }
            }
        }

        let mut camera_offset = authored_camera_offset;
        if third_person.catch_up_active {
            let (next_offset, next_velocity) = step_catch_up_offset(
                third_person.catch_up_offset_ls,
                third_person.catch_up_velocity_ls,
                authored_camera_offset,
                dt,
                config.third_person_catch_up_frequency_hz,
                config.third_person_catch_up_damping_ratio,
            );
            third_person.catch_up_offset_ls = next_offset;
            third_person.catch_up_velocity_ls = next_velocity;
            let settle_distance = config
                .third_person_catch_up_settle_distance_m
                .clamp(0.0, config.third_person_catch_up_max_distance_m.max(0.01));
            if (authored_camera_offset - next_offset).length() <= settle_distance {
                third_person.catch_up_offset_ls = authored_camera_offset;
                third_person.catch_up_velocity_ls = Vec3::ZERO;
                third_person.catch_up_active = false;
            }
            camera_offset = third_person.catch_up_offset_ls;
        }

        // Collision always evaluates the interpolated pre-collision frame. Interpolating a
        // post-collision position could otherwise sweep the camera through a wall during catch-up.
        let pre_collision_camera_ws = camera_target_position + target_rotation * camera_offset;
        third_person.last_pivot_ws = camera_target_position;
        third_person.last_focus_ws = focus_position;
        third_person.last_desired_camera_ws = pre_collision_camera_ws;
        let desired_arm_ws = pre_collision_camera_ws - focus_position;
        let desired_arm_distance = desired_arm_ws.length();
        third_person.last_collision_target_distance = desired_arm_distance;
        let mut collision_ratio = 1.0_f32;
        if desired_arm_distance > 1.0e-5 {
            let desired_arm_ls = target_rotation.inverse() * desired_arm_ws;
            let constrained_arm_ls = {
                let collision_world = world.resource::<CameraSpringArmCollisionWorld>();
                constrain_spring_arm_offset_ls(
                    player,
                    focus_position,
                    target_rotation,
                    desired_arm_ls,
                    CameraSpringArmConfig {
                        enabled: config.third_person_collision_enabled,
                        probe_radius: config.third_person_collision_probe_radius,
                        collision_padding: config.third_person_collision_padding,
                        min_distance: config.third_person_collision_min_distance,
                    },
                    collision_world,
                )
            };
            let collision_target_distance = constrained_arm_ls
                .length()
                .clamp(0.001, desired_arm_distance);
            third_person.last_collision_target_distance = collision_target_distance;
            let obstruction_epsilon = config
                .third_person_collision_distance_hysteresis
                .max(1.0e-4);
            let blocked_now =
                collision_target_distance < desired_arm_distance - obstruction_epsilon;
            if blocked_now {
                third_person.collision_blocked = true;
                let (collision_distance, collision_velocity) = step_collision_distance_response(
                    third_person.collision_distance,
                    third_person.collision_velocity,
                    collision_target_distance,
                    dt,
                    config.third_person_collision_release_frequency_hz,
                    config.third_person_collision_release_damping_ratio,
                    config.third_person_collision_distance_hysteresis,
                );
                third_person.collision_distance = collision_distance.min(desired_arm_distance);
                third_person.collision_velocity = collision_velocity;
            } else if third_person.collision_blocked {
                // Geometry has cleared. Only this path owns the authored damped pull-back. Changes
                // in zoom/catch-up arm length while already unobstructed must not synthesize a
                // second hidden lag layer.
                let (collision_distance, collision_velocity) = step_collision_distance_response(
                    third_person.collision_distance,
                    third_person.collision_velocity,
                    desired_arm_distance,
                    dt,
                    config.third_person_collision_release_frequency_hz,
                    config.third_person_collision_release_damping_ratio,
                    config.third_person_collision_distance_hysteresis,
                );
                third_person.collision_distance = collision_distance.min(desired_arm_distance);
                third_person.collision_velocity = collision_velocity;
                if desired_arm_distance - third_person.collision_distance
                    <= config
                        .third_person_collision_distance_hysteresis
                        .max(1.0e-4)
                {
                    third_person.collision_distance = desired_arm_distance;
                    third_person.collision_velocity = 0.0;
                    third_person.collision_blocked = false;
                }
            } else {
                third_person.collision_distance = desired_arm_distance;
                third_person.collision_velocity = 0.0;
            }
            collision_ratio =
                (third_person.collision_distance / desired_arm_distance).clamp(0.0, 1.0);
            let arm_dir_ws = desired_arm_ws / desired_arm_distance;
            let collision_safe_camera_ws =
                focus_position + arm_dir_ws * third_person.collision_distance;
            camera_offset =
                target_rotation.inverse() * (collision_safe_camera_ws - camera_target_position);
        }

        let next_pos = camera_target_position + target_rotation * camera_offset;
        let next_rot = if orbit_mode {
            // Orbit remains analytically centered; look damping must never introduce subject drift.
            let rotation = orbit_look_at_rotation(next_pos, focus_position);
            third_person.look_rotation = rotation;
            third_person.look_initialized = true;
            rotation
        } else if controller.follow_rotation {
            let desired_rotation =
                (target_rotation * controller.rot_offset).normalize_or_identity();
            let rotation = if third_person.look_initialized && third_person.catch_up_active {
                step_bounded_look_rotation(
                    third_person.look_rotation,
                    desired_rotation,
                    dt,
                    config.third_person_catch_up_frequency_hz * core::f32::consts::TAU,
                    core::f32::consts::PI,
                )
            } else {
                desired_rotation
            };
            third_person.look_rotation = rotation;
            third_person.look_initialized = true;
            rotation
        } else {
            // The ideal orientation is computed from the pre-collision camera frame. When collision
            // shortens the arm, progressively move the effective focus from the authored pivot
            // toward the collision root, then blend that response by project policy. This mirrors
            // the reference invariant without coupling camera runtime to gameplay object types.
            let desired_rotation = collision_aware_look_rotation(
                pre_collision_camera_ws,
                next_pos,
                camera_target_position,
                focus_position,
                collision_ratio,
                config.third_person_look_at_collision_blend,
            );
            let active_fov = match config.runner {
                GameplayCameraRunnerKind::ThirdPersonFollow => {
                    config.third_person_follow_fov_y_radians
                }
                GameplayCameraRunnerKind::ThirdPersonAim => config.third_person_aim_fov_y_radians,
                GameplayCameraRunnerKind::ThirdPersonOrbit => {
                    config.third_person_orbit_fov_y_radians
                }
                GameplayCameraRunnerKind::FirstPerson => config.first_person_fov_y_radians,
            };
            let max_error = 0.5
                * active_fov
                * config
                    .third_person_look_at_max_error_fov_fraction
                    .clamp(0.0, 1.0);
            let rotation = if third_person.look_initialized {
                step_bounded_look_rotation(
                    third_person.look_rotation,
                    desired_rotation,
                    dt,
                    config.third_person_look_at_response_hz,
                    max_error,
                )
            } else {
                desired_rotation
            };
            third_person.look_rotation = rotation;
            third_person.look_initialized = true;
            rotation
        };
        let _ = world.insert(camera, third_person);
        let _ = world.insert(
            camera,
            GameplayCameraRunnerHistory {
                target: player,
                runner: config.runner,
                initialized: true,
            },
        );

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
