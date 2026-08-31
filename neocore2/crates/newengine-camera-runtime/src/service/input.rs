use super::*;

#[inline]
fn apply_zoom_factor(
    controller: &mut FollowTargetCameraController,
    min_distance: f32,
    max_distance: f32,
    fallback_distance: f32,
    zoom_factor: f32,
) {
    let z = if controller.offset_ls.z.is_finite() {
        controller.offset_ls.z
    } else {
        fallback_distance
    };
    let sign = if z < 0.0 { -1.0 } else { 1.0 };
    let distance = z.abs().clamp(min_distance, max_distance);
    controller.offset_ls.z = sign * (distance * zoom_factor).clamp(min_distance, max_distance);
}

impl CameraRuntimeService {
    /// Applies mouse-wheel zoom to possessed third-person cameras. The wheel changes the
    /// desired spring-arm radius; render-cadence camera synchronization then moves toward it
    /// without teleporting the camera. Positive wheel delta zooms in.
    pub fn apply_gameplay_camera_zoom(
        world: &mut World,
        camera: EntityId,
        config: CameraRuntimeServiceConfig,
        wheel_y: f32,
    ) -> bool {
        let Some((min_distance, max_distance)) = gameplay_zoom_limits(config.runner) else {
            return false;
        };
        let steps = normalized_gameplay_zoom_steps(wheel_y);
        if steps.abs() <= f32::EPSILON {
            return false;
        }
        let Some(controller) = world.get_mut::<FollowTargetCameraController>(camera) else {
            return false;
        };

        // Exponential dolly keeps wheel response proportional and avoids overshoot near limits.
        apply_zoom_factor(
            controller,
            min_distance,
            max_distance,
            max_distance.min(4.0),
            (-steps * 0.16).exp(),
        );
        true
    }

    /// Applies MMB vertical drag as a dolly gesture for ThirdPersonOrbit.
    /// Dragging upward zooms in; dragging downward zooms out. This modifies the
    /// desired spring-arm radius, so collision and smooth zoom recovery remain authoritative.
    pub fn apply_gameplay_camera_drag_zoom(
        world: &mut World,
        camera: EntityId,
        config: CameraRuntimeServiceConfig,
        drag_dy_px: f32,
    ) -> bool {
        if !matches!(config.runner, GameplayCameraRunnerKind::ThirdPersonOrbit)
            || !drag_dy_px.is_finite()
            || drag_dy_px.abs() <= f32::EPSILON
        {
            return false;
        }
        let Some((min_distance, max_distance)) = gameplay_zoom_limits(config.runner) else {
            return false;
        };
        let Some(controller) = world.get_mut::<FollowTargetCameraController>(camera) else {
            return false;
        };
        // Screen-space Y grows downward. Negative dy therefore means an upward drag -> zoom in.
        // Exponential scaling keeps response proportional at both near and far radii.
        apply_zoom_factor(
            controller,
            min_distance,
            max_distance,
            GameplayThirdPersonOrbitRunner::default().orbit_offset.z,
            (drag_dy_px.clamp(-240.0, 240.0) * 0.008).exp(),
        );
        true
    }

    /// Applies free-orbit mouse look to the camera only. In ThirdPersonOrbit the
    /// CharacterMotor yaw/pitch and body transform are deliberately untouched.
    pub fn apply_gameplay_camera_orbit_look(
        world: &mut World,
        camera: EntityId,
        player: EntityId,
        config: CameraRuntimeServiceConfig,
        look_delta_px: Vec2,
        look_active: bool,
    ) -> bool {
        if !matches!(config.runner, GameplayCameraRunnerKind::ThirdPersonOrbit) || !look_active {
            return false;
        }
        let Some(controller) = world.get::<FollowTargetCameraController>(camera).copied() else {
            return false;
        };
        if controller.target != player {
            return false;
        }
        let delta = Vec2::new(
            if look_delta_px.x.is_finite() && look_delta_px.x.abs() >= 0.01 {
                look_delta_px.x
            } else {
                0.0
            },
            if look_delta_px.y.is_finite() && look_delta_px.y.abs() >= 0.01 {
                look_delta_px.y
            } else {
                0.0
            },
        );
        if delta.length_squared() <= 1.0e-12 {
            return false;
        }

        let (simulation_anchor_ws, simulation_body_rotation) =
            read_entity_world_pose_local_chain(world, player)
                .unwrap_or((Vec3::ZERO, Quat::IDENTITY));
        let anchor_ws = config
            .third_person_render_position_ws
            .filter(|position| position.is_finite())
            .unwrap_or(simulation_anchor_ws);
        let body_rotation = config
            .third_person_render_rotation_ws
            .filter(|rotation| rotation.is_finite())
            .unwrap_or(simulation_body_rotation)
            .normalize_or_identity();
        let pivot_offset_ls = if config.third_person_orbit_pivot_offset_ls.is_finite() {
            config.third_person_orbit_pivot_offset_ls
        } else {
            Vec3::ZERO
        };
        let pivot_ws = anchor_ws + body_rotation.normalize_or_identity() * pivot_offset_ls;
        let rig = world
            .get::<CameraRigComp>(camera)
            .copied()
            .unwrap_or_default()
            .0;

        let mut state = world
            .get::<GameplayThirdPersonCameraState>(camera)
            .copied()
            .unwrap_or_default();
        if !state.initialized || state.runner != config.runner || state.target != player {
            let inherited_view = world
                .get::<CharacterMotor>(player)
                .copied()
                .map(|motor| (wrap_pi(motor.yaw), motor.pitch.clamp(-1.35, 1.35)))
                .unwrap_or((0.0, 0.0));
            let (yaw, pitch) =
                orbit_angles_from_camera(pivot_ws, rig.position).unwrap_or(inherited_view);
            state.runner = config.runner;
            state.target = player;
            state.anchor_ws = anchor_ws;
            state.zoom_z = controller.offset_ls.z;
            state.orbit_yaw = yaw;
            state.orbit_pitch = pitch;
            state.orbit_pivot_offset_ws = body_rotation.normalize_or_identity() * pivot_offset_ls;
            state.collision_distance = 0.0;
            state.last_pivot_ws = pivot_ws;
            state.last_focus_ws = pivot_ws;
            state.last_desired_camera_ws = rig.position;
            state.last_collision_target_distance = 0.0;
            state.initialized = true;
        }

        // Orbit owns an independent camera angle after initialization. CharacterMotor is consulted only
        // as the stable entry orientation when the previous camera pose is at the orbit pole.
        const ORBIT_LOOK_SENSITIVITY: f32 = 0.0028;
        const ORBIT_PITCH_LIMIT: f32 = 1.35;
        state.orbit_yaw = wrap_pi(state.orbit_yaw + delta.x * ORBIT_LOOK_SENSITIVITY);
        state.orbit_pitch = (state.orbit_pitch + delta.y * ORBIT_LOOK_SENSITIVITY)
            .clamp(-ORBIT_PITCH_LIMIT, ORBIT_PITCH_LIMIT);
        let _ = world.insert(camera, state);
        true
    }

    fn apply_player_look_immediate(
        world: &mut World,
        player: EntityId,
        look_delta_px: Vec2,
        look_active: bool,
        runner: GameplayCameraRunnerKind,
        _first_person_body_yaw_limit_radians: f32,
        first_person_down_pitch_limit_radians: f32,
    ) -> bool {
        if !look_active {
            return false;
        }
        let delta = Vec2::new(
            if look_delta_px.x.is_finite() {
                look_delta_px.x
            } else {
                0.0
            },
            if look_delta_px.y.is_finite() {
                look_delta_px.y
            } else {
                0.0
            },
        );
        if delta.length_squared() <= 1.0e-12 {
            return false;
        }
        let Some(mut motor) = world.get::<CharacterMotor>(player).copied() else {
            return false;
        };
        let sensitivity = if motor.look_sens.is_finite() && motor.look_sens > 0.0 {
            motor.look_sens
        } else {
            CharacterMotor::default().look_sens
        };
        // View yaw is input-owned in every gameplay camera mode, including first person.
        // Never clamp it to body facing: authored head/eye pose-space consumes the range it can
        // represent and GameReady turns the body from the remaining residual. The camera remains
        // free while that body turn is in progress.
        motor.yaw = wrap_pi(motor.yaw + delta.x * sensitivity);
        motor.pitch += delta.y * sensitivity;
        let pitch_limit = if motor.pitch_limit.is_finite() && motor.pitch_limit > 0.0 {
            motor.pitch_limit
        } else {
            CharacterMotor::default().pitch_limit
        };
        let min_pitch = if matches!(runner, GameplayCameraRunnerKind::FirstPerson) {
            let authored_down_limit = if first_person_down_pitch_limit_radians.is_finite()
                && first_person_down_pitch_limit_radians > 0.0
            {
                first_person_down_pitch_limit_radians
                    .clamp(1.0_f32.to_radians(), 89.0_f32.to_radians())
            } else {
                85.0_f32.to_radians()
            };
            -authored_down_limit.min(pitch_limit)
        } else {
            -pitch_limit
        };
        motor.pitch = motor.pitch.clamp(min_pitch, pitch_limit);
        // Mouse-look owns the view orientation only. Do not write yaw/pitch back to
        // the PlayerActor transform: that transform represents body facing and is
        // driven by locomotion/aim at fixed-step cadence.
        let _ = world.insert(player, motor);
        true
    }

    /// Applies mouse look immediately at render/input cadence while movement
    /// remains fixed-step deterministic. This removes fixed-step quantization and
    /// one-frame latency from first-person camera rotation.
    pub fn apply_player_input(
        world: &mut World,
        player: EntityId,
        input_mask: u64,
        look_delta_px: Vec2,
        look_active: bool,
        sprint_multiplier: f32,
        runner: GameplayCameraRunnerKind,
        first_person_body_yaw_limit_radians: f32,
        first_person_down_pitch_limit_radians: f32,
        face_view: bool,
    ) {
        let mut axis = Vec3::ZERO;
        if input_mask & input_move::FORWARD != 0 {
            axis.z += 1.0;
        }
        if input_mask & input_move::BACK != 0 {
            axis.z -= 1.0;
        }
        if input_mask & input_move::RIGHT != 0 {
            axis.x += 1.0;
        }
        if input_mask & input_move::LEFT != 0 {
            axis.x -= 1.0;
        }
        if input_mask & input_move::UP != 0 {
            axis.y += 1.0;
        }
        if input_mask & input_move::DOWN != 0 {
            axis.y -= 1.0;
        }

        // ThirdPersonOrbit owns yaw/pitch in camera state. It must never mutate
        // CharacterMotor view orientation, because that would also rotate locomotion
        // basis and eventually the visible character body.
        let player_look_active =
            look_active && !matches!(runner, GameplayCameraRunnerKind::ThirdPersonOrbit);
        let player_look_delta = if player_look_active {
            look_delta_px
        } else {
            Vec2::ZERO
        };
        let look_applied_immediately = Self::apply_player_look_immediate(
            world,
            player,
            player_look_delta,
            player_look_active,
            runner,
            first_person_body_yaw_limit_radians,
            first_person_down_pitch_limit_radians,
        );

        if let Some(input) = world.get_mut::<MotorInput>(player) {
            input.move_axis = axis;
            input.look_delta = if look_applied_immediately {
                Vec2::ZERO
            } else {
                player_look_delta
            };
            input.look_active = player_look_active && !look_applied_immediately;
            input.speed_mul = if input_mask & input_move::SPRINT != 0 {
                sprint_multiplier.max(1.0)
            } else {
                1.0
            };
            input.zoom_delta = 0.0;
            input.face_view = face_view;
        }
    }
}
