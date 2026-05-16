#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_sim::{CameraRigComp, FollowTargetCameraController, FollowTargetCameraMotor};
use newengine_transform_api::Transform;

use crate::service::{CameraRuntimeService, CameraRuntimeServiceConfig};

/// Runtime session mode consumed by the camera-runtime layer.
///
/// This deliberately does not depend on gameplay crates. The host maps its own
/// play-state enum into this compact session contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CameraRuntimeSessionMode {
    #[default]
    Edit,
    Simulate,
    Play,
}

impl CameraRuntimeSessionMode {
    #[inline]
    pub const fn is_runtime(self) -> bool {
        matches!(self, Self::Simulate | Self::Play)
    }

    #[inline]
    pub const fn wants_direct_player_control(self) -> bool {
        matches!(self, Self::Play)
    }
}

/// Camera-owned session snapshot used when a runtime director temporarily possesses
/// the runtime camera.
#[derive(Clone, Copy, Debug)]
pub struct CameraPlaySessionSnapshot {
    pub cam_id: EntityId,
    pub rig: CameraRigComp,
    pub transform: Option<Transform>,
    pub follow_controller: Option<FollowTargetCameraController>,
    pub follow_motor: Option<FollowTargetCameraMotor>,
}

/// ECS resource storing camera-session state. This replaces render-controller fields
/// such as `last_play_mode` and `PlaySessionSnapshot`.
#[derive(Clone, Debug)]
pub struct CameraRuntimeSessionResource {
    pub last_mode: CameraRuntimeSessionMode,
    pub play_camera: Option<CameraPlaySessionSnapshot>,
}

impl Default for CameraRuntimeSessionResource {
    #[inline]
    fn default() -> Self {
        Self {
            last_mode: CameraRuntimeSessionMode::Edit,
            play_camera: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraRuntimeSessionSync {
    pub camera: EntityId,
    pub player: Option<EntityId>,
    pub mode: CameraRuntimeSessionMode,
    pub service_config: CameraRuntimeServiceConfig,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraRuntimeSessionReport {
    pub previous_mode: CameraRuntimeSessionMode,
    pub current_mode: CameraRuntimeSessionMode,
    pub entered_runtime: bool,
    pub left_runtime: bool,
    pub entered_direct_player_control: bool,
    pub left_direct_player_control: bool,
    pub captured_camera: bool,
    pub restored_camera: bool,
    pub possessed_player: bool,
    pub released_player: bool,
}

pub struct CameraRuntimeSessionService;

impl CameraRuntimeSessionService {
    #[inline]
    pub fn ensure_resource(world: &mut World) {
        if world.resource::<CameraRuntimeSessionResource>().is_none() {
            world.insert_resource(CameraRuntimeSessionResource::default());
        }
    }

    #[inline]
    pub fn current_mode(world: &World) -> CameraRuntimeSessionMode {
        world
            .resource::<CameraRuntimeSessionResource>()
            .map(|session| session.last_mode)
            .unwrap_or_default()
    }

    pub fn sync(world: &mut World, request: CameraRuntimeSessionSync) -> CameraRuntimeSessionReport {
        Self::ensure_resource(world);

        let previous_mode = Self::current_mode(world);
        let entered_runtime = !previous_mode.is_runtime() && request.mode.is_runtime();
        let left_runtime = previous_mode.is_runtime() && !request.mode.is_runtime();
        let entered_direct_player_control = !previous_mode.wants_direct_player_control()
            && request.mode.wants_direct_player_control();
        let left_direct_player_control = previous_mode.wants_direct_player_control()
            && !request.mode.wants_direct_player_control();

        let mut report = CameraRuntimeSessionReport {
            previous_mode,
            current_mode: request.mode,
            entered_runtime,
            left_runtime,
            entered_direct_player_control,
            left_direct_player_control,
            captured_camera: false,
            restored_camera: false,
            possessed_player: false,
            released_player: false,
        };

        if left_direct_player_control {
            if let Some(player) = request.player {
                CameraRuntimeService::clear_player_input(world, player);
            }
            report.released_player = CameraRuntimeService::release_player(world, request.camera);
            report.restored_camera = Self::restore_play_camera(world);
        }

        if entered_direct_player_control {
            report.captured_camera = Self::capture_play_camera(world, request.camera);
            if let Some(player) = request.player {
                report.possessed_player = CameraRuntimeService::possess_player(
                    world,
                    request.camera,
                    player,
                    request.service_config,
                );
            }
        }

        if !request.mode.wants_direct_player_control() {
            if let Some(player) = request.player {
                CameraRuntimeService::clear_player_input(world, player);
            }
        }

        if let Some(session) = world.resource_mut::<CameraRuntimeSessionResource>() {
            session.last_mode = request.mode;
        }

        report
    }

    fn capture_play_camera(world: &mut World, camera: EntityId) -> bool {
        if !world.exists(camera) {
            return false;
        }
        let rig = world
            .get::<CameraRigComp>(camera)
            .copied()
            .unwrap_or_default();
        let transform = world.get::<Transform>(camera).copied();
        let follow_controller = world.get::<FollowTargetCameraController>(camera).copied();
        let follow_motor = world.get::<FollowTargetCameraMotor>(camera).copied();

        if let Some(session) = world.resource_mut::<CameraRuntimeSessionResource>() {
            session.play_camera = Some(CameraPlaySessionSnapshot {
                cam_id: camera,
                rig,
                transform,
                follow_controller,
                follow_motor,
            });
            true
        } else {
            false
        }
    }

    fn restore_play_camera(world: &mut World) -> bool {
        let snapshot = {
            let Some(session) = world.resource_mut::<CameraRuntimeSessionResource>() else {
                return false;
            };
            session.play_camera.take()
        };
        let Some(snapshot) = snapshot else {
            return false;
        };
        if !world.exists(snapshot.cam_id) {
            return false;
        }

        let _ = world.insert(snapshot.cam_id, snapshot.rig);
        if let Some(transform) = snapshot.transform {
            let _ = world.insert(snapshot.cam_id, transform);
        } else {
            let _ = world.remove::<Transform>(snapshot.cam_id);
        }
        if let Some(ctrl) = snapshot.follow_controller {
            let _ = world.insert(snapshot.cam_id, ctrl);
        } else {
            let _ = world.remove::<FollowTargetCameraController>(snapshot.cam_id);
        }
        if let Some(motor) = snapshot.follow_motor {
            let _ = world.insert(snapshot.cam_id, motor);
        } else {
            let _ = world.remove::<FollowTargetCameraMotor>(snapshot.cam_id);
        }
        true
    }
}
