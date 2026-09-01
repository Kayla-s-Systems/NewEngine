use super::*;

#[inline]
fn first_person_horizontal_forward(rotation: Quat) -> Vec3 {
    let forward = (rotation.normalize_or_identity() * -Vec3::Z).normalize_or_zero();
    Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero()
}

#[inline]
fn first_person_position_contract(
    body_rotation: Quat,
    _camera_rotation: Quat,
    forward_clearance: f32,
) -> (Vec3, Vec3) {
    let mut body_forward = first_person_horizontal_forward(body_rotation);
    if body_forward.length_squared() <= 1.0e-8 {
        body_forward = Vec3::new(0.0, 0.0, -1.0);
    }

    // The gameplay eye is a rigid body-relative anchor. Mouse yaw/pitch must never translate it.
    // Previous centimetre-scale parallax was visually harmless in isolation, but in full-body FPP
    // it changed the direction fed into the self/body constraints and turned tiny look changes into
    // visible positional pops. Any head/weapon parallax belongs to presentation, not camera position.
    (body_forward * forward_clearance, Vec3::ZERO)
}

#[inline]
fn first_person_ads_position_contract(
    hip_camera_position: Vec3,
    ads_camera_position: Option<Vec3>,
    aim_alpha: f32,
) -> Vec3 {
    let aim_alpha = if aim_alpha.is_finite() {
        aim_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    ads_camera_position
        .filter(|position| position.is_finite())
        .map(|ads| hip_camera_position.lerp(ads, aim_alpha))
        .unwrap_or(hip_camera_position)
}

#[inline]
fn constrain_first_person_camera_position(
    player: EntityId,
    eye_center: Vec3,
    desired_camera_position: Vec3,
    spring_arm: CameraSpringArmConfig,
    collision_world: Option<&CameraSpringArmCollisionWorld>,
) -> Vec3 {
    let desired_offset_ws = desired_camera_position - eye_center;
    if !desired_offset_ws.is_finite() || desired_offset_ws.length_squared() <= 1.0e-10 {
        return eye_center;
    }
    // FPP uses the same collision scene as the third-person spring arm but with a head-sized
    // probe and no minimum arm length. The PlayerActor collider itself is ignored by the shared
    // constraint, so this prevents wall/ceiling penetration without treating the player's own
    // capsule as an obstacle.
    let constrained_offset_ws = constrain_spring_arm_offset_ls(
        player,
        eye_center,
        Quat::IDENTITY,
        desired_offset_ws,
        spring_arm,
        collision_world,
    );
    let constrained = eye_center + constrained_offset_ws;
    if constrained.is_finite() {
        constrained
    } else {
        eye_center
    }
}

fn stabilize_first_person_eye_anchor(
    current: Vec3,
    target: Vec3,
    grounded: bool,
    dt: f32,
    deadband_m: f32,
    time_constant_seconds: f32,
) -> Vec3 {
    if !target.is_finite() {
        return current;
    }
    if !current.is_finite() {
        return target;
    }

    // X/Z already come from render-cadence player presentation and must remain spatially exact.
    // Only grounded Y receives hysteresis because physics contact/grounding can oscillate by a few
    // millimetres even while the character is visually standing still.
    let mut next = Vec3::new(target.x, current.y, target.z);
    if !grounded {
        next.y = target.y;
        return next;
    }

    let deadband_m = if deadband_m.is_finite() {
        deadband_m.clamp(0.0, 0.25)
    } else {
        0.010
    };
    let time_constant_seconds = if time_constant_seconds.is_finite() {
        time_constant_seconds.clamp(0.001, 5.0)
    } else {
        0.060
    };
    let delta = target.y - current.y;
    if delta.abs() <= deadband_m {
        return next;
    }
    if !(dt.is_finite() && dt > 0.0) {
        next.y = target.y;
        return next;
    }
    let outside_deadband = delta - delta.signum() * deadband_m;
    let alpha = (1.0 - (-dt.min(0.05) / time_constant_seconds).exp()).clamp(0.0, 1.0);
    next.y = current.y + outside_deadband * alpha;
    next
}

#[derive(Clone, Copy, Debug, Default)]
struct FirstPersonAdditivePose {
    position_ls: Vec3,
    rotation_ls: Quat,
}

#[inline]
fn signed_sequence_noise(sequence: u64, salt: u64) -> f32 {
    let bits = (newengine_math::avalanche_u64(sequence ^ salt) >> 40) as u32 & 0x00ff_ffff;
    (bits as f32 / 0x00ff_ffffu32 as f32) * 2.0 - 1.0
}

fn step_first_person_additive_motion(
    state: &mut GameplayFirstPersonCameraState,
    input: FirstPersonPresentationInput,
    dt: f32,
    aim_response_hz: f32,
    camera_recoil_share: f32,
) -> FirstPersonAdditivePose {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.05)
    } else {
        0.0
    };
    let aim_target = if input.aiming { 1.0 } else { 0.0 };
    if dt > 0.0 {
        let aim_response_hz = if aim_response_hz.is_finite() {
            aim_response_hz.clamp(0.01, 120.0)
        } else {
            18.0
        };
        let alpha = 1.0 - (-aim_response_hz * dt).exp();
        state.aim_alpha =
            (state.aim_alpha + (aim_target - state.aim_alpha) * alpha).clamp(0.0, 1.0);
    } else {
        state.aim_alpha = aim_target;
    }

    if input.shot_sequence != state.last_shot_sequence {
        const PITCH_SALT: u64 = 0x243f_6a88_85a3_08d3;
        const YAW_SALT: u64 = 0x1319_8a2e_0370_7344;
        let pitch_base = input.recoil_pitch_radians.max(0.0);
        let pitch_random = input.recoil_pitch_random_radians.max(0.0);
        let yaw_random = input.recoil_yaw_radians.max(0.0);
        let yaw_bias = if input.recoil_yaw_bias_radians.is_finite() {
            input.recoil_yaw_bias_radians
        } else {
            0.0
        };
        let ads_multiplier = if input.ads_recoil_multiplier.is_finite() {
            input.ads_recoil_multiplier.clamp(0.0, 4.0)
        } else {
            1.0
        };
        let recoil_scale = 1.0 + (ads_multiplier - 1.0) * state.aim_alpha;
        // Camera recoil is intentionally separate from weapon-side recoil. The project camera
        // definition owns the visual share while the runtime owns impulse execution/recovery.
        let camera_recoil_share = if camera_recoil_share.is_finite() {
            camera_recoil_share.clamp(0.0, 2.0)
        } else {
            0.42
        };
        state.recoil_pitch_radians += (pitch_base
            + signed_sequence_noise(input.shot_sequence, PITCH_SALT) * pitch_random)
            .max(0.0)
            * recoil_scale
            * camera_recoil_share;
        state.recoil_yaw_radians += (yaw_bias
            + signed_sequence_noise(input.shot_sequence, YAW_SALT) * yaw_random)
            * recoil_scale
            * camera_recoil_share;
        state.last_shot_sequence = input.shot_sequence;
    }

    if dt > 0.0 {
        let recovery_hz = if input.recoil_recovery_hz.is_finite() {
            input.recoil_recovery_hz.clamp(0.05, 120.0)
        } else {
            7.5
        };
        let decay = (-recovery_hz * dt).exp();
        state.recoil_pitch_radians *= decay;
        state.recoil_yaw_radians *= decay;
    }
    state.recoil_pitch_radians = state.recoil_pitch_radians.clamp(0.0, 0.20);
    state.recoil_yaw_radians = state.recoil_yaw_radians.clamp(-0.12, 0.12);

    // Locomotion never modifies the gameplay camera transform. Full-body motion is already visible
    // on the animated body and weapon; duplicating gait as camera pitch/roll is perceived as
    // camera bounce and makes the hidden-head boundary easier to expose. Only authored recoil is
    // allowed to affect the first-person view rotation.

    FirstPersonAdditivePose {
        // Full-body FPP keeps the eye position locked to the stable render-cadence anchor.
        // Locomotion belongs to the body/weapon presentation; translating the camera itself makes
        // the body barrier amplify millimetre-scale bob into visible lateral/vertical jumps.
        position_ls: Vec3::ZERO,
        rotation_ls: (Quat::from_rotation_y(state.recoil_yaw_radians)
            * Quat::from_rotation_x(state.recoil_pitch_radians))
        .normalize_or_identity(),
    }
}

#[inline]
fn step_catch_up_offset(
    current: Vec3,
    velocity: Vec3,
    target: Vec3,
    dt: f32,
    frequency_hz: f32,
    damping_ratio: f32,
) -> (Vec3, Vec3) {
    if !current.is_finite() || !velocity.is_finite() || !target.is_finite() {
        return (target, Vec3::ZERO);
    }
    if !(dt.is_finite() && dt > 0.0) {
        return (target, Vec3::ZERO);
    }
    let frequency_hz = if frequency_hz.is_finite() {
        frequency_hz.clamp(0.01, 60.0)
    } else {
        2.4
    };
    let damping_ratio = if damping_ratio.is_finite() {
        damping_ratio.clamp(0.05, 4.0)
    } else {
        1.0
    };
    let dt = dt.min(0.05);
    let omega = core::f32::consts::TAU * frequency_hz;
    let f = 1.0 + 2.0 * dt * damping_ratio * omega;
    let omega_sq = omega * omega;
    let h_omega_sq = dt * omega_sq;
    let hh_omega_sq = dt * h_omega_sq;
    let inv_det = (f + hh_omega_sq).recip();
    let mut next = (current * f + velocity * dt + target * hh_omega_sq) * inv_det;
    let mut next_velocity = (velocity + (target - current) * h_omega_sq) * inv_det;
    if !next.is_finite() || !next_velocity.is_finite() {
        return (target, Vec3::ZERO);
    }
    // A catch-up trajectory may approach the authored relative frame but never cross it and
    // oscillate around the player. Collision is evaluated after this step.
    if (target - current).dot(target - next) <= 0.0 {
        next = target;
        next_velocity = Vec3::ZERO;
    }
    (next, next_velocity)
}

#[inline]
fn collision_aware_look_rotation(
    pre_collision_camera_ws: Vec3,
    collision_safe_camera_ws: Vec3,
    camera_target_position: Vec3,
    focus_position: Vec3,
    collision_ratio: f32,
    collision_blend: f32,
) -> Quat {
    let pre_collision_rotation = orbit_look_at_rotation(pre_collision_camera_ws, focus_position);
    let collision_ratio = if collision_ratio.is_finite() {
        collision_ratio.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let collision_focus = camera_target_position.lerp(focus_position, collision_ratio);
    let post_collision_rotation = orbit_look_at_rotation(collision_safe_camera_ws, collision_focus);
    let collision_blend = if collision_blend.is_finite() {
        collision_blend.clamp(0.0, 1.0)
    } else {
        0.0
    };
    pre_collision_rotation
        .slerp(post_collision_rotation, collision_blend)
        .normalize_or_identity()
}

#[inline]
fn step_bounded_look_rotation(
    current: Quat,
    desired: Quat,
    dt: f32,
    response_hz: f32,
    max_error_radians: f32,
) -> Quat {
    let desired = desired.normalize_or_identity();
    if !current.is_finite() || !(dt.is_finite() && dt > 0.0) {
        return desired;
    }
    let current = current.normalize_or_identity();
    let max_error = if max_error_radians.is_finite() {
        max_error_radians.clamp(0.0, core::f32::consts::PI)
    } else {
        0.0
    };
    if max_error <= 1.0e-6 {
        return desired;
    }
    let dot = current.dot(desired).abs().clamp(0.0, 1.0);
    let error = 2.0 * dot.acos();
    let bounded_current = if error > max_error && error > 1.0e-6 {
        desired.slerp(current, (max_error / error).clamp(0.0, 1.0))
    } else {
        current
    };
    let response_hz = if response_hz.is_finite() {
        response_hz.clamp(0.01, 120.0)
    } else {
        14.0
    };
    let alpha = (1.0 - (-response_hz * dt.min(0.05)).exp()).clamp(0.0, 1.0);
    bounded_current
        .slerp(desired, alpha)
        .normalize_or_identity()
}

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
