struct CameraGatewayState {
    nav: newengine_camera_runtime::CameraNavState,
    last_play_mode: GameRunMode,
    play_session: Option<CameraPlaySessionSnapshot>,
    last_snapshot: Option<CameraFrameSnapshot>,
    active_view: CameraViewMode,
    authored_initial_view_applied: bool,
}

#[inline]
fn authored_camera_view_mode(
    view: newengine_gameplay_world_runtime::gameplay::PlayerCameraViewMode,
) -> CameraViewMode {
    match view {
        newengine_gameplay_world_runtime::gameplay::PlayerCameraViewMode::FirstPerson => {
            CameraViewMode::FirstPerson
        }
        newengine_gameplay_world_runtime::gameplay::PlayerCameraViewMode::ThirdPersonFollow => {
            CameraViewMode::ThirdPersonFollow
        }
        newengine_gameplay_world_runtime::gameplay::PlayerCameraViewMode::ThirdPersonAim => {
            CameraViewMode::ThirdPersonAim
        }
        newengine_gameplay_world_runtime::gameplay::PlayerCameraViewMode::ThirdPersonOrbit => {
            CameraViewMode::ThirdPersonOrbit
        }
    }
}

fn parse_camera_start_view(raw: Option<&str>) -> CameraViewMode {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("follow" | "thirdpersonfollow" | "third_person_follow") => {
            CameraViewMode::ThirdPersonFollow
        }
        Some("aim" | "thirdpersonaim" | "third_person_aim") => CameraViewMode::ThirdPersonAim,
        Some("orbit" | "thirdpersonorbit" | "third_person_orbit") => {
            CameraViewMode::ThirdPersonOrbit
        }
        _ => CameraViewMode::FirstPerson,
    }
}

impl Default for CameraGatewayState {
    #[inline]
    fn default() -> Self {
        let start_view = newengine_runtime_env::var("NEWENGINE_CAMERA_START_VIEW");
        Self {
            nav: newengine_camera_runtime::CameraNavState::default(),
            last_play_mode: GameRunMode::Staging,
            play_session: None,
            last_snapshot: None,
            active_view: parse_camera_start_view(start_view.as_deref()),
            authored_initial_view_applied: false,
        }
    }
}

impl CameraGatewayState {
    fn apply_authored_initial_view(&mut self, world: &World) {
        if self.authored_initial_view_applied {
            return;
        }
        let Some(profile) = first_player(world)
            .and_then(|player| {
                world.get::<newengine_gameplay_world_runtime::gameplay::PlayerCameraProfile>(player)
            })
            .copied()
        else {
            return;
        };
        self.active_view = authored_camera_view_mode(profile.initial_view);
        self.authored_initial_view_applied = true;
    }

    fn set_view_command(&mut self, command: CameraViewCommand) -> CameraViewMode {
        self.active_view = match command {
            CameraViewCommand::Next => self.active_view.next(),
            CameraViewCommand::Previous => self.active_view.previous(),
            CameraViewCommand::Set(mode) => mode,
        };
        self.active_view
    }

    fn apply_input_view_request(&mut self, request: CameraViewRequest) -> CameraViewMode {
        match request {
            CameraViewRequest::None => self.active_view,
            CameraViewRequest::Next => self.set_view_command(CameraViewCommand::Next),
            CameraViewRequest::Previous => self.set_view_command(CameraViewCommand::Previous),
            CameraViewRequest::Set(mode) => self.set_view_command(CameraViewCommand::Set(mode)),
        }
    }

    fn sync_play_mode_transition(
        &mut self,
        world: &mut World,
        cam_id: EntityId,
        effective_play_mode: GameRunMode,
    ) {
        if self.last_play_mode == effective_play_mode {
            return;
        }

        if self.last_play_mode.wants_direct_player_control() {
            if let Some(player) = first_player(world) {
                CameraRuntimeService::clear_player_input(world, player);
            }
            if let Some(snapshot) = self.play_session.take() {
                let _ = world.insert(snapshot.cam_id, snapshot.rig);
                if let Some(transform) = snapshot.transform {
                    let _ = world.insert(snapshot.cam_id, transform);
                }
            }
        }

        if effective_play_mode.wants_direct_player_control() {
            let rig = world
                .get::<newengine_sim::CameraRigComp>(cam_id)
                .copied()
                .unwrap_or_default();
            let transform = world.get::<Transform>(cam_id).copied();
            self.play_session = Some(CameraPlaySessionSnapshot {
                cam_id,
                rig,
                transform,
            });
        }

        self.last_play_mode = effective_play_mode;
    }
}

#[derive(Clone, Copy, Debug)]
struct CameraPlaySessionSnapshot {
    cam_id: EntityId,
    rig: newengine_sim::CameraRigComp,
    transform: Option<Transform>,
}
