#[derive(Clone, Debug, Default)]
pub struct CameraGatewayInput {
    pub dx_px: f32,
    pub dy_px: f32,
    pub wheel_y: f32,
    pub active: bool,
    pub look_drag: bool,
    pub pan_drag: bool,
    pub ui_busy: bool,
    pub fly_rmb: bool,
    pub sampling_alive: bool,
    pub camera_navigation_gated: bool,
    pub gameplay_movement_gated: bool,
    pub move_mask: u64,
    pub speed_scalar: f32,
    pub camera_view: CameraViewRequest,
    pub gameplay_actions: ActionCommandFrame,
}

#[derive(Clone, Debug)]
pub struct CameraGatewayFrame {
    pub frame_index: u64,
    pub camera_snapshot: CameraFrameSnapshot,
    pub view: EngineViewFrame,
    pub postfx: ViewPostFxFrameParams,
    pub report: Option<CameraRuntimeOverlayReport>,
    pub cursor: CursorState,
    pub effective_play_mode: GameRunMode,
    pub world_playable: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct EngineViewFrame {
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub inverse_view: Mat4,
    pub position_ws: Vec3,
    pub position_ws_f64: [f64; 3],
    pub world_origin_ws_f64: [f64; 3],
    pub position_origin_relative_ws: Vec3,
    pub forward_ws: Vec3,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub aspect: f32,
}

#[cfg(test)]
mod camera_gateway_start_view_tests {
    use super::*;

    #[test]
    fn camera_start_view_defaults_to_first_person() {
        assert_eq!(parse_camera_start_view(None), CameraViewMode::FirstPerson);
        assert_eq!(
            parse_camera_start_view(Some("unknown")),
            CameraViewMode::FirstPerson
        );
    }

    #[test]
    fn camera_start_view_can_force_orbit_for_runtime_diagnostics() {
        assert_eq!(
            parse_camera_start_view(Some("orbit")),
            CameraViewMode::ThirdPersonOrbit
        );
        assert_eq!(
            parse_camera_start_view(Some("third_person_orbit")),
            CameraViewMode::ThirdPersonOrbit
        );
    }

    #[test]
    fn authored_project_camera_overrides_transient_engine_start_view_once() {
        let mut world = World::new();
        let player = world.spawn();
        let _ = world.insert(
            player,
            newengine_gameplay_world_runtime::gameplay::PlayerActor,
        );
        let mut profile =
            newengine_gameplay_world_runtime::gameplay::PlayerCameraProfile::default();
        profile.initial_view =
            newengine_gameplay_world_runtime::gameplay::PlayerCameraViewMode::ThirdPersonOrbit;
        let _ = world.insert(player, profile);
        let mut state = CameraGatewayState::default();
        state.active_view = CameraViewMode::FirstPerson;
        state.apply_authored_initial_view(&world);
        assert_eq!(state.active_view, CameraViewMode::ThirdPersonOrbit);
        assert!(state.authored_initial_view_applied);

        profile.initial_view =
            newengine_gameplay_world_runtime::gameplay::PlayerCameraViewMode::ThirdPersonAim;
        let _ = world.insert(player, profile);
        state.apply_authored_initial_view(&world);
        assert_eq!(
            state.active_view,
            CameraViewMode::ThirdPersonOrbit,
            "runtime view changes must not be reset every frame by authored startup policy"
        );
    }

    #[test]
    fn weaponless_fire_and_aim_intent_cannot_activate_camera_ads() {
        let mut world = World::new();
        let player = world.spawn();
        let mut commands = PlayerCommandFrame::default();
        commands.actions.held.push("player.fire.primary".to_owned());
        commands.actions.held.push("player.aim".to_owned());
        let _ = world.insert(player, commands);

        assert!(!active_weapon_aim_intent(&world, player));
    }

    #[test]
    fn camera_snapshot_request_rejects_missing_authoritative_frame() {
        let mut state = CameraGatewayState::default();
        match invoke_camera_gateway(&mut state, Blob::from(Vec::<u8>::new())) {
            RResult::RErr(error) => assert!(error
                .to_string()
                .contains("authoritative camera snapshot unavailable")),
            RResult::ROk(_) => panic!("missing authoritative camera must not fall back to default"),
        }
    }
}
