use super::*;

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
}
