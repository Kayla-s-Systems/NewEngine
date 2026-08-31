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

fn stabilize_first_person_eye_anchor(current: Vec3, target: Vec3, grounded: bool, dt: f32) -> Vec3 {
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

    const GROUNDED_EYE_Y_DEADBAND_M: f32 = 0.010;
    const GROUNDED_EYE_Y_TIME_CONSTANT_SEC: f32 = 0.060;
    let delta = target.y - current.y;
    if delta.abs() <= GROUNDED_EYE_Y_DEADBAND_M {
        return next;
    }
    if !(dt.is_finite() && dt > 0.0) {
        next.y = target.y;
        return next;
    }
    let outside_deadband = delta - delta.signum() * GROUNDED_EYE_Y_DEADBAND_M;
    let alpha = (1.0 - (-dt.min(0.05) / GROUNDED_EYE_Y_TIME_CONSTANT_SEC).exp()).clamp(0.0, 1.0);
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
) -> FirstPersonAdditivePose {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.05)
    } else {
        0.0
    };
    let aim_target = if input.aiming { 1.0 } else { 0.0 };
    if dt > 0.0 {
        let aim_response_hz = 18.0_f32;
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
        // Camera recoil is intentionally smaller than weapon-side recoil. The weapon animation is
        // the primary visual kick; the camera receives a separate additive impulse like REDengine.
        const CAMERA_RECOIL_SHARE: f32 = 0.42;
        state.recoil_pitch_radians += (pitch_base
            + signed_sequence_noise(input.shot_sequence, PITCH_SALT) * pitch_random)
            .max(0.0)
            * recoil_scale
            * CAMERA_RECOIL_SHARE;
        state.recoil_yaw_radians += (yaw_bias
            + signed_sequence_noise(input.shot_sequence, YAW_SALT) * yaw_random)
            * recoil_scale
            * CAMERA_RECOIL_SHARE;
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
                );
            }
            let eye_center = first_person.stable_eye_anchor_ws;
            let additive = step_first_person_additive_motion(
                &mut first_person,
                config.first_person_presentation,
                dt,
            );
            let desired_camera_position =
                eye_center + base_offset + camera_rotation * additive.position_ls;

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
        let next = stabilize_first_person_eye_anchor(current, target, true, 1.0 / 60.0);
        assert!((next.x - target.x).abs() <= 1.0e-8);
        assert!((next.z - target.z).abs() <= 1.0e-8);
        assert!((next.y - current.y).abs() <= 1.0e-8);
    }

    #[test]
    fn meaningful_grounded_eye_height_change_converges_without_teleport() {
        let current = Vec3::new(0.0, 1.62, 0.0);
        let target = Vec3::new(0.0, 1.42, 0.0);
        let next = stabilize_first_person_eye_anchor(current, target, true, 1.0 / 60.0);
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
        let _ = step_first_person_additive_motion(&mut state, shot, 1.0 / 120.0);
        let first = state.recoil_pitch_radians;
        let _ = step_first_person_additive_motion(&mut state, shot, 1.0 / 120.0);
        let second = state.recoil_pitch_radians;
        assert!(first > 0.0);
        assert!(
            second < first,
            "same shot sequence must decay, not inject twice"
        );
        for _ in 0..240 {
            let _ = step_first_person_additive_motion(&mut state, shot, 1.0 / 120.0);
        }
        assert!(state.recoil_pitch_radians < second * 0.01);
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
