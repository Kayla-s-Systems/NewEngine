#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::CameraRig;
use newengine_core::host_events::CursorState;
use newengine_ecs::{EntityId, World};
use newengine_input_actions_api::move_mask as input_move;
use newengine_math::{wrap_pi, EulerRot, Mat3, Quat, Vec2, Vec3};
use newengine_sim::{
    step_follow_camera, CameraRigComp, CharacterMotor, FollowTargetCameraController,
    FollowTargetCameraMotor, MotorInput,
};
use newengine_transform::{
    read_entity_world_pose_local_chain, write_entity_local_from_world_pose_local_chain,
};

use crate::constraints::{
    constrain_spring_arm_offset_ls, CameraSpringArmCollisionWorld, CameraSpringArmConfig,
};
use crate::manager::{CameraDirectorRequest, CameraManagerResource};
use crate::modes::{
    GameplayFirstPersonRunner, GameplayThirdPersonAimRunner, GameplayThirdPersonFollowRunner,
    GameplayThirdPersonOrbitRunner,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameplayCameraRunnerKind {
    FirstPerson,
    ThirdPersonFollow,
    ThirdPersonAim,
    ThirdPersonOrbit,
}

#[derive(Clone, Copy, Debug)]
struct GameplayThirdPersonCameraState {
    runner: GameplayCameraRunnerKind,
    target: EntityId,
    anchor_ws: Vec3,
    zoom_z: f32,
    orbit_yaw: f32,
    orbit_pitch: f32,
    orbit_pivot_offset_ws: Vec3,
    collision_distance: f32,
    last_pivot_ws: Vec3,
    last_focus_ws: Vec3,
    last_desired_camera_ws: Vec3,
    last_collision_target_distance: f32,
    initialized: bool,
}

impl Default for GameplayThirdPersonCameraState {
    #[inline]
    fn default() -> Self {
        Self {
            runner: GameplayCameraRunnerKind::ThirdPersonFollow,
            target: EntityId::default(),
            anchor_ws: Vec3::ZERO,
            zoom_z: 0.0,
            orbit_yaw: 0.0,
            orbit_pitch: 0.0,
            orbit_pivot_offset_ws: Vec3::ZERO,
            collision_distance: 0.0,
            last_pivot_ws: Vec3::ZERO,
            last_focus_ws: Vec3::ZERO,
            last_desired_camera_ws: Vec3::ZERO,
            last_collision_target_distance: 0.0,
            initialized: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GameplayCameraTelemetrySnapshot {
    pub runner: GameplayCameraRunnerKind,
    pub target: EntityId,
    pub initialized: bool,
    pub anchor_ws: Vec3,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub orbit_pivot_offset_ws: Vec3,
    pub zoom_z: f32,
    pub collision_distance: f32,
    pub pivot_ws: Vec3,
    pub focus_ws: Vec3,
    pub desired_camera_ws: Vec3,
    pub collision_target_distance: f32,
    pub rig_position_ws: Vec3,
    pub rig_rotation: Quat,
}

#[inline]
fn normalized_gameplay_zoom_steps(wheel_y: f32) -> f32 {
    if !wheel_y.is_finite() || wheel_y.abs() <= f32::EPSILON {
        return 0.0;
    }
    if wheel_y.abs() > 10.0 {
        (wheel_y / 120.0).clamp(-4.0, 4.0)
    } else {
        wheel_y.clamp(-4.0, 4.0)
    }
}

#[inline]
fn gameplay_zoom_limits(runner: GameplayCameraRunnerKind) -> Option<(f32, f32)> {
    match runner {
        GameplayCameraRunnerKind::FirstPerson => None,
        GameplayCameraRunnerKind::ThirdPersonAim => Some((1.10, 4.50)),
        GameplayCameraRunnerKind::ThirdPersonFollow => Some((1.35, 9.00)),
        GameplayCameraRunnerKind::ThirdPersonOrbit => Some((1.35, 10.0)),
    }
}

#[inline]
fn orbit_angles_from_camera(pivot_ws: Vec3, camera_ws: Vec3) -> Option<(f32, f32)> {
    let dir = (camera_ws - pivot_ws).normalize_or_zero();
    if !dir.is_finite() || dir.length_squared() <= 1.0e-10 {
        return None;
    }
    // A first-person eye is commonly almost directly above the third-person pivot.
    // Yaw is undefined at that pole: atan2(tiny_x, tiny_z) amplifies sub-pixel pose noise
    // into arbitrary left/right orbit angles. Let the caller inherit the gameplay view instead.
    let horizontal_sq = dir.x * dir.x + dir.z * dir.z;
    if horizontal_sq <= 0.05 * 0.05 {
        return None;
    }
    let yaw = dir.x.atan2(dir.z);
    let pitch = (-dir.y).asin().clamp(-1.35, 1.35);
    Some((wrap_pi(yaw), pitch))
}

#[inline]
fn orbit_look_at_rotation(eye: Vec3, center: Vec3) -> Quat {
    let forward = (center - eye).normalize_or_zero();
    if forward.length_squared() <= 1.0e-12 {
        return Quat::IDENTITY;
    }
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    if right.length_squared() <= 1.0e-12 {
        return Quat::IDENTITY;
    }
    let up = right.cross(forward).normalize_or_zero();
    Quat::from_mat3(&Mat3::from_cols(right, up, -forward)).normalize_or_identity()
}

#[inline]
fn smooth_gameplay_zoom(current: f32, target: f32, dt: f32) -> f32 {
    if !target.is_finite() {
        return current;
    }
    if !current.is_finite() || !(dt.is_finite() && dt > 0.0) {
        return target;
    }
    let alpha = (1.0 - (-dt / 0.09).exp()).clamp(0.0, 1.0);
    let next = current + (target - current) * alpha;
    if (target - next).abs() <= 1.0e-4 {
        target
    } else {
        next
    }
}

#[inline]
fn smooth_collision_release(current: f32, target: f32, dt: f32) -> f32 {
    if !target.is_finite() || target <= 0.0 {
        return current;
    }
    if !current.is_finite() || current <= 0.0 {
        return target;
    }
    // Triangle seams/contact quantization can move the measured hit distance by a few
    // millimetres from frame to frame. Reacting asymmetrically (instant retract, soft release)
    // turns that harmless noise into visible camera sway. The spring arm already keeps 8 cm of
    // authored collision padding, so a 1 cm distance deadband is safely inside that margin.
    const COLLISION_DISTANCE_HYSTERESIS: f32 = 0.010;
    if (target - current).abs() <= COLLISION_DISTANCE_HYSTERESIS {
        return current;
    }
    // Meaningful collision retraction is immediate so the camera never clips into geometry.
    if target < current {
        return target;
    }
    if !(dt.is_finite() && dt > 0.0) {
        return target;
    }
    // Release is intentionally softer to avoid a hard pop when a wall leaves the arm.
    let alpha = (1.0 - (-dt / 0.12).exp()).clamp(0.0, 1.0);
    let next = current + (target - current) * alpha;
    if (target - next).abs() <= 1.0e-4 {
        target
    } else {
        next
    }
}

impl Default for GameplayCameraRunnerKind {
    #[inline]
    fn default() -> Self {
        Self::FirstPerson
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraRuntimeServiceConfig {
    pub runner: GameplayCameraRunnerKind,
    pub first_person_eye_height: f32,
    /// Optional animated eye-center anchor supplied by the active avatar provider.
    pub first_person_anchor_ws: Option<Vec3>,
    /// Forward clearance from the eye center. This keeps the near plane in front of face geometry.
    pub first_person_forward_clearance: f32,
    /// Visual character center relative to the PlayerActor root.
    /// ThirdPersonOrbit uses this exact point as both orbit pivot and look-at target.
    pub third_person_orbit_pivot_offset_ls: Vec3,
    /// Optional render-cadence interpolated target pose supplied by the engine presentation layer.
    pub third_person_render_position_ws: Option<Vec3>,
    pub third_person_render_rotation_ws: Option<Quat>,
    pub sprint_multiplier: f32,
}

impl Default for CameraRuntimeServiceConfig {
    #[inline]
    fn default() -> Self {
        Self {
            runner: GameplayCameraRunnerKind::FirstPerson,
            first_person_eye_height: 1.6,
            first_person_anchor_ws: None,
            first_person_forward_clearance: 0.055,
            third_person_orbit_pivot_offset_ls: Vec3::ZERO,
            third_person_render_position_ws: None,
            third_person_render_rotation_ws: None,
            sprint_multiplier: 2.0,
        }
    }
}

pub struct CameraRuntimeService;

impl CameraRuntimeService {
    #[inline]
    pub fn gameplay_camera_telemetry(
        world: &World,
        camera: EntityId,
    ) -> Option<GameplayCameraTelemetrySnapshot> {
        let state = world
            .get::<GameplayThirdPersonCameraState>(camera)
            .copied()?;
        let rig = world.get::<CameraRigComp>(camera).copied()?.0;
        Some(GameplayCameraTelemetrySnapshot {
            runner: state.runner,
            target: state.target,
            initialized: state.initialized,
            anchor_ws: state.anchor_ws,
            orbit_yaw: state.orbit_yaw,
            orbit_pitch: state.orbit_pitch,
            orbit_pivot_offset_ws: state.orbit_pivot_offset_ws,
            zoom_z: state.zoom_z,
            collision_distance: state.collision_distance,
            pivot_ws: state.last_pivot_ws,
            focus_ws: state.last_focus_ws,
            desired_camera_ws: state.last_desired_camera_ws,
            collision_target_distance: state.last_collision_target_distance,
            rig_position_ws: rig.position,
            rig_rotation: rig.rotation,
        })
    }

    #[inline]
    pub fn ensure_manager_resource(world: &mut World) {
        if world.resource::<CameraManagerResource>().is_none() {
            world.insert_resource(CameraManagerResource::default());
        }
    }

    pub fn apply_pending_director_requests(
        world: &mut World,
        camera: EntityId,
        config: CameraRuntimeServiceConfig,
    ) {
        loop {
            let request = {
                let Some(manager) = world.resource_mut::<CameraManagerResource>() else {
                    return;
                };
                manager.take_pending_request()
            };
            let Some(request) = request else {
                break;
            };
            Self::apply_director_request(world, camera, request, config);
        }
    }

    #[inline]
    pub fn apply_director_request(
        world: &mut World,
        camera: EntityId,
        request: CameraDirectorRequest,
        config: CameraRuntimeServiceConfig,
    ) -> bool {
        match request {
            CameraDirectorRequest::PossessPlayer { player } => {
                Self::possess_player(world, camera, player, config)
            }
            CameraDirectorRequest::ReleasePlayer => Self::release_player(world, camera),
        }
    }

    pub fn possess_player(
        world: &mut World,
        camera: EntityId,
        player: EntityId,
        config: CameraRuntimeServiceConfig,
    ) -> bool {
        if !world.exists(camera) || !world.exists(player) {
            return false;
        }

        let follow = match config.runner {
            GameplayCameraRunnerKind::FirstPerson => GameplayFirstPersonRunner {
                eye_height: config.first_person_eye_height,
            }
            .controller(player),
            GameplayCameraRunnerKind::ThirdPersonFollow => {
                GameplayThirdPersonFollowRunner::default().controller(player)
            }
            GameplayCameraRunnerKind::ThirdPersonAim => {
                GameplayThirdPersonAimRunner::default().controller(player)
            }
            GameplayCameraRunnerKind::ThirdPersonOrbit => {
                GameplayThirdPersonOrbitRunner::default().controller(player)
            }
        };

        let _ = world.insert(camera, follow);
        let _ = world.insert(camera, FollowTargetCameraMotor::default());

        if world.get::<CameraRigComp>(camera).is_none() {
            let _ = world.insert(camera, CameraRigComp(CameraRig::default()));
        }

        true
    }

    #[inline]
    pub fn release_player(world: &mut World, camera: EntityId) -> bool {
        let removed_follow = world
            .remove::<newengine_sim::FollowTargetCameraController>(camera)
            .is_some();
        let removed_motor = world.remove::<FollowTargetCameraMotor>(camera).is_some();
        let _ = world.remove::<GameplayThirdPersonCameraState>(camera);
        removed_follow || removed_motor
    }

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

        let z = if controller.offset_ls.z.is_finite() {
            controller.offset_ls.z
        } else {
            max_distance.min(4.0)
        };
        let sign = if z < 0.0 { -1.0 } else { 1.0 };
        let distance = z.abs().clamp(min_distance, max_distance);
        // Exponential dolly keeps wheel response proportional and avoids overshoot near limits.
        let zoom_factor = (-steps * 0.16).exp();
        controller.offset_ls.z = sign * (distance * zoom_factor).clamp(min_distance, max_distance);
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
        let z = if controller.offset_ls.z.is_finite() {
            controller.offset_ls.z
        } else {
            GameplayThirdPersonOrbitRunner::default().orbit_offset.z
        };
        let sign = if z < 0.0 { -1.0 } else { 1.0 };
        let distance = z.abs().clamp(min_distance, max_distance);
        // Screen-space Y grows downward. Negative dy therefore means an upward drag -> zoom in.
        // Exponential scaling keeps response proportional at both near and far radii.
        let zoom_factor = (drag_dy_px.clamp(-240.0, 240.0) * 0.008).exp();
        controller.offset_ls.z = sign * (distance * zoom_factor).clamp(min_distance, max_distance);
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
        motor.yaw += delta.x * sensitivity;
        motor.pitch += delta.y * sensitivity;
        let pitch_limit = if motor.pitch_limit.is_finite() && motor.pitch_limit > 0.0 {
            motor.pitch_limit
        } else {
            CharacterMotor::default().pitch_limit
        };
        motor.pitch = motor.pitch.clamp(-pitch_limit, pitch_limit);
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
        let look_applied_immediately =
            Self::apply_player_look_immediate(world, player, player_look_delta, player_look_active);

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

    /// Compatibility wrapper for callers that explicitly require first-person semantics.
    #[inline]
    pub fn sync_first_person_camera_now(
        world: &mut World,
        camera: EntityId,
        player: EntityId,
    ) -> bool {
        let Some(controller) = world.get::<FollowTargetCameraController>(camera).copied() else {
            return false;
        };
        if !controller.follow_rotation {
            return false;
        }
        let config = CameraRuntimeServiceConfig {
            runner: GameplayCameraRunnerKind::FirstPerson,
            first_person_eye_height: controller.offset_ls.y.max(0.01),
            ..CameraRuntimeServiceConfig::default()
        };
        Self::sync_gameplay_camera_now(world, camera, player, config, 1.0 / 60.0)
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
mod tests {
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
            let _ =
                CameraRuntimeService::apply_gameplay_camera_zoom(&mut world, camera, config, 4.0);
        }
        let min_zoom = world
            .get::<FollowTargetCameraController>(camera)
            .unwrap()
            .offset_ls
            .z;
        assert!((min_zoom - 1.35).abs() < 1.0e-4);

        for _ in 0..64 {
            let _ =
                CameraRuntimeService::apply_gameplay_camera_zoom(&mut world, camera, config, -4.0);
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
}
