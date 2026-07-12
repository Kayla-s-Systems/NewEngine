#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::CameraRig;
use newengine_core::host_events::CursorState;
use newengine_ecs::{EntityId, World};
use newengine_input_actions_api::move_mask as input_move;
use newengine_math::{EulerRot, Quat, Vec2, Vec3};
use newengine_sim::{
    CameraRigComp, CharacterMotor, FollowTargetCameraController, FollowTargetCameraMotor,
    MotorInput,
};
use newengine_transform::{
    read_entity_world_pose_local_chain, write_entity_local_from_world_pose_local_chain,
};

use crate::manager::{CameraDirectorRequest, CameraManagerResource};
use crate::modes::{
    GameplayFirstPersonRunner, GameplayThirdPersonAimRunner, GameplayThirdPersonFollowRunner,
};

#[derive(Clone, Copy, Debug)]
pub enum GameplayCameraRunnerKind {
    FirstPerson,
    ThirdPersonFollow,
    ThirdPersonAim,
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
    pub sprint_multiplier: f32,
}

impl Default for CameraRuntimeServiceConfig {
    #[inline]
    fn default() -> Self {
        Self {
            runner: GameplayCameraRunnerKind::FirstPerson,
            first_person_eye_height: 1.6,
            sprint_multiplier: 2.0,
        }
    }
}

pub struct CameraRuntimeService;

impl CameraRuntimeService {
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
        removed_follow || removed_motor
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
        let rotation = Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0);
        let position = read_entity_world_pose_local_chain(world, player)
            .map(|(position, _)| position)
            .unwrap_or(Vec3::ZERO);
        let _ = world.insert(player, motor);
        write_entity_local_from_world_pose_local_chain(world, player, position, rotation);
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

        let look_applied_immediately =
            Self::apply_player_look_immediate(world, player, look_delta_px, look_active);

        if let Some(input) = world.get_mut::<MotorInput>(player) {
            input.move_axis = axis;
            input.look_delta = if look_applied_immediately {
                Vec2::ZERO
            } else {
                look_delta_px
            };
            input.look_active = look_active && !look_applied_immediately;
            input.speed_mul = if input_mask & input_move::SPRINT != 0 {
                sprint_multiplier.max(1.0)
            } else {
                1.0
            };
            input.zoom_delta = 0.0;
        }
    }

    /// Synchronizes the first-person camera to the already updated player
    /// pose in the same render frame. Fixed-step simulation remains authoritative
    /// for translation; view rotation no longer waits for the next simulation tick.
    pub fn sync_first_person_camera_now(
        world: &mut World,
        camera: EntityId,
        player: EntityId,
    ) -> bool {
        let Some(controller) = world.get::<FollowTargetCameraController>(camera).copied() else {
            return false;
        };
        if controller.target != player || !controller.follow_rotation {
            return false;
        }
        let Some((target_position, target_rotation)) =
            read_entity_world_pose_local_chain(world, player)
        else {
            return false;
        };
        let target_rotation = target_rotation.normalize_or_identity();
        let camera_position = target_position + target_rotation * controller.offset_ls;
        let camera_rotation = (target_rotation * controller.rot_offset).normalize_or_identity();
        let _ = world.insert(
            camera,
            CameraRigComp(CameraRig {
                position: camera_position,
                rotation: camera_rotation,
            }),
        );
        let _ = world.insert(camera, FollowTargetCameraMotor::default());
        write_entity_local_from_world_pose_local_chain(
            world,
            camera,
            camera_position,
            camera_rotation,
        );
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
