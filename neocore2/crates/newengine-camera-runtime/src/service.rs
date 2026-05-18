#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::CameraRig;
use newengine_core::host_events::CursorState;
use newengine_ecs::{EntityId, World};
use newengine_math::{Vec2, Vec3};
use newengine_sim::{
    CameraRigComp, FollowTargetCameraMotor, MotorInput,
};
use newengine_input_bindings::move_mask as input_move;

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
        let removed_follow = world.remove::<newengine_sim::FollowTargetCameraController>(camera).is_some();
        let removed_motor = world.remove::<FollowTargetCameraMotor>(camera).is_some();
        removed_follow || removed_motor
    }

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

        if let Some(input) = world.get_mut::<MotorInput>(player) {
            input.move_axis = axis;
            input.look_delta = look_delta_px;
            input.look_active = look_active;
            input.speed_mul = if input_mask & input_move::SPRINT != 0 {
                sprint_multiplier.max(1.0)
            } else {
                1.0
            };
            input.zoom_delta = 0.0;
        }
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
