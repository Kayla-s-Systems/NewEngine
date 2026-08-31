#![forbid(unsafe_op_in_unsafe_fn)]

use parking_lot::Mutex;
use std::sync::Arc;

use abi_stable::std_types::{RResult, RString};

use newengine_camera::{
    CameraChannel, CameraChannelState, CameraFrame, CameraViewport, Projection,
    RuntimeNavController, RuntimeNavMode,
};
use newengine_camera_api::{
    CameraServiceInfo, CameraViewCommand, CameraViewCommandRequest, CameraViewCommandResponse,
    CameraViewMode, CAMERA_BACKEND_CAPABILITY_ID, ENGINE_CAMERA_SERVICE_ID,
};
use newengine_camera_contracts::CameraFrameSnapshot;
use newengine_camera_runtime::{
    camera_frame_snapshot_for_view, cursor_state_for_nav, step_camera_nav,
    BoundsSphere as CamBoundsSphere, CameraManagerResource, CameraNavFrameRequest, CameraNavInput,
    CameraNavParams, CameraRuntimeReport, CameraRuntimeService, CameraRuntimeServiceConfig,
    CameraRuntimeWorldState, CameraSpringArmAabbCollider, CameraSpringArmCollisionWorld,
    CameraSpringArmMeshCollider, CameraTransitionPhase as RuntimeCameraTransitionPhase,
};
use newengine_core::host_events::CursorState;
use newengine_core::render::{
    PostFxFrameParams, ViewDepthOfFieldFrameParams, ViewMotionBlurFrameParams,
    ViewPostFxFrameParams,
};
use newengine_ecs::{EntityId, World};
use newengine_input_actions_api::{ActionCommandFrame, CameraViewRequest};
use newengine_math::{Mat4, Vec2, Vec3};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    decode_json_payload, engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use newengine_transform::Transform;

use crate::engine_bounds::EngineBoundsSnap;
use crate::gameplay::{
    active_equipped_weapon_aiming, active_equipped_weapon_can_aim, apply_player_command_frame,
    emit_player_event, first_player, is_player_controller_enabled, sync_player_view_listeners,
    CharacterBody, CharacterMotionTuning, GameRunMode, PlayerCommandFrame, PlayerEventKind,
    PlayerStanceState,
};
use newengine_viewport_bridge::ViewportBridge;

const CAMERA_GATEWAY_OWNER: &str = "newengine-engine-runtime.camera-gateway";
const CAMERA_GATEWAY_ROUTE: &str = "engine.camera.stargazer";

#[path = "camera_gateway_helpers.rs"]
mod camera_gateway_helpers;
use self::camera_gateway_helpers::{
    apply_gameplay_view_lens, apply_runtime_input, camera_nav_input, camera_report_snapshot,
    camera_runtime_service_config, follow_controller_offset_z,
    refresh_camera_spring_arm_collision_world, route_player_input_channels, sanitize_camera_dt,
    trace_gameplay_camera_frame, view_postfx_from_camera_snapshot,
};
pub use self::camera_gateway_helpers::{
    apply_view_postfx, CameraRuntimeOverlayReport, CameraTransitionOverlayReport,
    CameraTransitionPhase,
};

fn camera_gateway_service(
    state: Arc<Mutex<CameraGatewayState>>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = CameraServiceInfo::default();
    let description = engine_gateway_provider_service_description(
        ENGINE_CAMERA_SERVICE_ID,
        CAMERA_GATEWAY_OWNER,
        CAMERA_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .gateway("engine.camera.stargazer in-process bridge");

    JsonServiceRouter::with_shared_state(ENGINE_CAMERA_SERVICE_ID, state)
        .describe_json(&description)
        .info(CameraServiceInfo::default)
        .blob(
            newengine_camera_api::CAMERA_SERVICE_METHOD_SNAPSHOT_JSON_V1,
            camera_snapshot_gateway,
        )
        .blob(
            newengine_camera_api::CAMERA_SERVICE_METHOD_INVOKE,
            invoke_camera_gateway,
        )
        .blob(
            newengine_camera_api::CAMERA_SERVICE_METHOD_VIEW_SET_JSON_V1,
            apply_camera_view_command,
        )
        .get_json(
            newengine_camera_api::CAMERA_SERVICE_METHOD_VIEW_NEXT_JSON_V1,
            |state| {
                let active_view = state.set_view_command(CameraViewCommand::Next);
                CameraViewCommandResponse { active_view }
            },
        )
        .blob(
            newengine_camera_api::CAMERA_SERVICE_METHOD_SHUTDOWN_V1,
            |_state, _payload| ok_empty_blob(),
        )
        .into_service_v1()
}

fn authoritative_camera_snapshot(
    state: &CameraGatewayState,
) -> Result<CameraFrameSnapshot, RString> {
    state
        .last_snapshot
        .ok_or_else(|| RString::from("engine.camera: authoritative camera snapshot unavailable"))
}

fn camera_snapshot_gateway(
    state: &mut CameraGatewayState,
    _payload: Blob,
) -> RResult<Blob, RString> {
    match authoritative_camera_snapshot(state) {
        Ok(snapshot) => ok_json(snapshot),
        Err(error) => RResult::RErr(error),
    }
}

fn invoke_camera_gateway(state: &mut CameraGatewayState, payload: Blob) -> RResult<Blob, RString> {
    if payload.as_slice().is_empty() {
        return camera_snapshot_gateway(state, payload);
    }
    apply_camera_view_command(state, payload)
}

fn apply_camera_view_command(
    state: &mut CameraGatewayState,
    payload: Blob,
) -> RResult<Blob, RString> {
    if payload.is_empty() {
        return RResult::RErr(RString::from(
            "engine.camera: view_set_json_v1 requires CameraViewCommandRequest JSON",
        ));
    }
    let request = match decode_json_payload::<CameraViewCommandRequest>(
        ENGINE_CAMERA_SERVICE_ID,
        newengine_camera_api::CAMERA_SERVICE_METHOD_VIEW_SET_JSON_V1,
        &payload,
    ) {
        Ok(req) => req,
        Err(e) => return RResult::RErr(e),
    };
    let active_view = state.set_view_command(request.command);
    ok_json(CameraViewCommandResponse { active_view })
}

pub(crate) fn camera_gateway_route_is_authoritative_in_current_host_context() -> bool {
    newengine_plugin_host::active_engine_gateway_route(ENGINE_CAMERA_SERVICE_ID).is_some_and(
        |route| {
            route.provider_service_id == ENGINE_CAMERA_SERVICE_ID
                && route.provider_route_id.as_deref() == Some(CAMERA_GATEWAY_ROUTE)
                && route.provider_owner_id == CAMERA_GATEWAY_OWNER
                && route.backend_capability_id == CAMERA_BACKEND_CAPABILITY_ID
        },
    )
}

fn register_camera_gateway_service_best_effort(state: Arc<Mutex<CameraGatewayState>>) -> bool {
    // Gateway registries are HostContext-scoped. The registry itself is therefore
    // the idempotency authority; a process-global once flag would incorrectly
    // suppress publication when a runtime switches to a fresh HostContext.
    if camera_gateway_route_is_authoritative_in_current_host_context() {
        return true;
    }

    let service = camera_gateway_service(state);
    if register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_CAMERA_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Camera,
        provider_service: ENGINE_CAMERA_SERVICE_ID,
        provider_route: CAMERA_GATEWAY_ROUTE,
        capability: CAMERA_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: CAMERA_GATEWAY_OWNER,
        service,
    })
    .is_ok()
    {
        return true;
    }

    // Another publication racing this call may have won after our initial probe.
    camera_gateway_route_is_authoritative_in_current_host_context()
}

/// Runtime-hosted camera gateway bridge.
///
/// This is the in-process runtime implementation of the `engine.camera` boundary
/// until camera providers are moved behind `camera.api` service plugins. Render
/// code talks to this bridge for a resolved view frame; it does not own camera
/// navigation state and does not import camera runtime crates directly.
pub struct CameraGatewayBridge {
    state: Arc<Mutex<CameraGatewayState>>,
}

impl CameraGatewayBridge {
    #[inline]
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(CameraGatewayState::default()));
        let bridge = Self { state };
        let _ = bridge.publish_gateway_best_effort();
        bridge
    }

    /// Publishes this bridge into the gateway registry of the currently active HostContext.
    ///
    /// This is intentionally repeatable because runtime launchers create instance-owned
    /// HostContexts after profile construction. Re-publication is a no-op when the
    /// authoritative camera route is already present in the active context.
    pub fn publish_gateway_best_effort(&self) -> bool {
        register_camera_gateway_service_best_effort(Arc::clone(&self.state))
    }

    /// Samples direct-player input before the fixed simulation schedule.
    ///
    /// Camera/view requests are consumed here exactly once. The render-phase camera
    /// resolution then observes the post-simulation player pose without adding an
    /// extra frame of movement/look latency.
    pub fn prepare_world_input(
        &self,
        world: &mut World,
        input: CameraGatewayInput,
        effective_play_mode: GameRunMode,
        frame_index: u64,
    ) {
        let active_view = self
            .state
            .lock()
            .apply_input_view_request(input.camera_view);
        // World-runtime presentation (including the first-person weapon) runs before
        // `tick_world_frame`. Publish the active view here as well so the viewmodel never consumes
        // a stale third-person/first-person state for the current render frame.
        sync_player_view_listeners(world, matches!(active_view, CameraViewMode::FirstPerson));
        let service_config = camera_runtime_service_config(world, active_view);
        apply_runtime_input(
            world,
            input,
            effective_play_mode,
            service_config,
            frame_index,
        );
    }

    pub fn tick_world_frame(
        &self,
        world: &mut World,
        viewport: &ViewportBridge,
        input: CameraGatewayInput,
        play_mode: GameRunMode,
        effective_play_mode: GameRunMode,
        world_playable: bool,
        frame_index: u64,
        dt: f32,
        vp_w: u32,
        vp_h: u32,
        bounds: EngineBoundsSnap,
        selection_bounds: Option<EngineBoundsSnap>,
    ) -> CameraGatewayFrame {
        let camera_dt = sanitize_camera_dt(dt);
        let mut state = self.state.lock();
        let mut nav_input = camera_nav_input(input.clone(), play_mode);
        let active_view = state.apply_input_view_request(input.camera_view);
        sync_player_view_listeners(world, matches!(active_view, CameraViewMode::FirstPerson));
        let cam_id = world
            .resource::<newengine_scene::SceneState>()
            .and_then(|s| s.active_camera.or(s.root))
            .unwrap_or_default();

        CameraRuntimeService::ensure_manager_resource(world);

        let existing_nav_mode = world
            .get::<RuntimeNavController>(cam_id)
            .map(|ctrl| ctrl.mode)
            .unwrap_or(RuntimeNavMode::Orbit);
        let player = first_player(world);
        let camera_profile = player
            .and_then(|player| world.get::<crate::gameplay::PlayerCameraProfile>(player))
            .copied()
            .map(crate::gameplay::PlayerCameraProfile::sanitized);
        let gate_blocked = play_mode.is_runtime() && !world_playable;
        let mut controller_z_phases = [f32::NAN; 5];
        controller_z_phases[0] = follow_controller_offset_z(world, cam_id);

        let suppress_game_nav = {
            let manager = world
                .resource_mut::<CameraManagerResource>()
                .expect("camera manager resource inserted");
            if let Some(profile) = camera_profile {
                manager.settings.gameplay.blend_in_sec = profile.gameplay_blend_in_seconds;
                manager.settings.gameplay.blend_out_sec = profile.gameplay_blend_out_seconds;
                manager.settings.gameplay.lock_input = profile.gameplay_blend_lock_input;
            }
            manager.advance(camera_dt);
            manager.sync_world_state(CameraRuntimeWorldState {
                game_nav_mode: existing_nav_mode,
                runtime_requested: play_mode.is_runtime(),
                public_runtime_active: effective_play_mode.is_runtime(),
                wants_direct_player_control: effective_play_mode.wants_direct_player_control(),
                gate_blocked,
                player,
                view_mode: active_view,
            });
            !manager.wants_navigation_input()
        };

        state.sync_play_mode_transition(world, cam_id, effective_play_mode);
        let service_config = camera_runtime_service_config(world, active_view);
        CameraRuntimeService::apply_pending_director_requests(world, cam_id, service_config);
        controller_z_phases[1] = follow_controller_offset_z(world, cam_id);
        let gameplay_capture = crate::gameplay::gameplay_input_capture(world);
        let mut routed_camera_input_for_trace = None;
        if effective_play_mode.wants_direct_player_control() {
            if let Some(player) = player {
                let routed_camera_input = route_player_input_channels(&input, gameplay_capture);
                routed_camera_input_for_trace = Some(routed_camera_input);
                refresh_camera_spring_arm_collision_world(world, player);
                let orbit_dolly_drag = matches!(
                    service_config.runner,
                    newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonOrbit
                ) && input.pan_drag;
                if !orbit_dolly_drag {
                    let _ = CameraRuntimeService::apply_gameplay_camera_orbit_look(
                        world,
                        cam_id,
                        player,
                        service_config,
                        routed_camera_input.look_delta,
                        routed_camera_input.look_active,
                    );
                }

                // Wheel/MMB dolly are gameplay-camera channels. Runtime navigation is deliberately
                // gated while the player owns the camera, so consume the wheel here before the
                // generic nav path zeros it. UI/script camera capture still blocks the zoom.
                if !input.camera_navigation_gated && !gameplay_capture.block_camera_navigation {
                    let _ = CameraRuntimeService::apply_gameplay_camera_zoom(
                        world,
                        cam_id,
                        service_config,
                        input.wheel_y,
                    );
                    if orbit_dolly_drag {
                        let _ = CameraRuntimeService::apply_gameplay_camera_drag_zoom(
                            world,
                            cam_id,
                            service_config,
                            input.dy_px,
                        );
                    }
                }

                // Gameplay view rotation is sampled at render cadence. Third-person cameras
                // smooth only their player anchor; angular orbit remains render-cadence direct.
                controller_z_phases[2] = follow_controller_offset_z(world, cam_id);
                let _ = CameraRuntimeService::sync_gameplay_camera_now(
                    world,
                    cam_id,
                    player,
                    service_config,
                    camera_dt,
                );
                controller_z_phases[3] = follow_controller_offset_z(world, cam_id);
            }
        }

        let params = CameraNavParams {
            dt: camera_dt,
            viewport: CameraViewport::from_size(vp_w, vp_h),
            channel: CameraChannelState::dominant(if effective_play_mode.is_runtime() {
                CameraChannel::Gameplay
            } else {
                CameraChannel::Runtime
            }),
            bounds: CamBoundsSphere {
                center: bounds.center,
                radius: bounds.radius,
            },
            selection_bounds: selection_bounds.map(|b| CamBoundsSphere {
                center: b.center,
                radius: b.radius,
            }),
        };

        let frame_req = CameraNavFrameRequest {
            seq: viewport.read_frame_request(),
            all: viewport.read_frame_all(),
        };

        if suppress_game_nav
            || effective_play_mode.wants_direct_player_control()
            || nav_input.navigation_gated
            || gameplay_capture.block_camera_navigation
        {
            nav_input.gate_navigation();
        }

        let out = step_camera_nav(
            &mut state.nav,
            world,
            cam_id,
            &mut nav_input,
            params,
            frame_req,
        );
        controller_z_phases[4] = follow_controller_offset_z(world, cam_id);

        let first_person_aiming =
            player.is_some_and(|player| active_weapon_aim_intent(world, player));
        let (snapshot, report, resolved_frame) =
            if let Some(manager) = world.resource_mut::<CameraManagerResource>() {
                manager.sync_runtime_nav_mode_from_controller(out.controller.mode);
                manager.set_last_cursor(out.cursor);
                let frame = manager.resolve_camera_frame(out.frame, dt);
                let frame = apply_gameplay_view_lens(
                    frame,
                    manager.active_view_mode(),
                    first_person_aiming,
                    service_config,
                );
                let effects = manager.last_post_effects().unwrap_or_default();
                (
                    camera_frame_snapshot_for_view(frame, effects, manager.active_view_mode()),
                    Some(camera_report_snapshot(manager.report())),
                    frame,
                )
            } else {
                (
                    camera_frame_snapshot_for_view(out.frame, Default::default(), active_view),
                    None,
                    out.frame,
                )
            };

        trace_gameplay_camera_frame(
            frame_index,
            dt,
            &input,
            routed_camera_input_for_trace,
            active_view,
            world,
            player,
            cam_id,
            out.frame,
            resolved_frame,
            report.as_ref(),
            controller_z_phases,
        );

        if let Some(listener) =
            newengine_audio_client::audio_listener_from_camera_snapshot(&snapshot)
        {
            world.insert_resource(newengine_audio_world_api::AudioListenerRuntimeState {
                listener,
                frame_index,
            });
        }
        newengine_audio_client::sync_audio_listener_from_camera_snapshot(&snapshot);
        debug_assert!(
            snapshot.finite,
            "authoritative camera snapshot must be finite"
        );
        debug_assert!(
            snapshot
                .view_cols
                .iter()
                .flatten()
                .all(|value| value.is_finite()),
            "authoritative camera view matrix must be finite"
        );
        debug_assert!(
            snapshot
                .projection_cols
                .iter()
                .flatten()
                .all(|value| value.is_finite()),
            "authoritative camera projection matrix must be finite"
        );
        debug_assert!(
            snapshot.position_ws.iter().all(|value| value.is_finite()),
            "authoritative camera position must be finite"
        );
        state.last_snapshot = Some(snapshot);
        let view = EngineViewFrame::from_camera_snapshot(snapshot);
        let cursor = if effective_play_mode.wants_direct_player_control()
            && input.active
            && !input.camera_navigation_gated
            && !gameplay_capture.release_cursor
        {
            CursorState::captured_locked()
        } else {
            cursor_state_for_nav(&nav_input)
        };

        CameraGatewayFrame {
            frame_index,
            camera_snapshot: snapshot,
            view,
            postfx: view_postfx_from_camera_snapshot(snapshot),
            report,
            cursor,
            effective_play_mode,
            world_playable,
        }
    }
}

#[inline]
fn active_weapon_aim_intent(world: &World, player: EntityId) -> bool {
    // Input actions are intent only. ADS/FOV may exist only when the authoritative active weapon
    // explicitly exposes the Aim capability. This keeps raw mouse input from becoming a camera
    // zoom for Unarmed, melee, an empty/incomplete equipment state, or a stale weapon command.
    active_equipped_weapon_can_aim(world, player)
        && (world
            .get::<PlayerCommandFrame>(player)
            .is_some_and(|commands| commands.actions.is_held("player.aim"))
            || active_equipped_weapon_aiming(world, player))
}

impl Default for CameraGatewayBridge {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

struct CameraGatewayState {
    nav: newengine_camera_runtime::CameraNavState,
    last_play_mode: GameRunMode,
    play_session: Option<CameraPlaySessionSnapshot>,
    last_snapshot: Option<CameraFrameSnapshot>,
    active_view: CameraViewMode,
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
        let start_view = crate::env_config::var("NEWENGINE_CAMERA_START_VIEW");
        Self {
            nav: newengine_camera_runtime::CameraNavState::default(),
            last_play_mode: GameRunMode::Staging,
            play_session: None,
            last_snapshot: None,
            active_view: parse_camera_start_view(start_view.as_deref()),
        }
    }
}

impl CameraGatewayState {
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
