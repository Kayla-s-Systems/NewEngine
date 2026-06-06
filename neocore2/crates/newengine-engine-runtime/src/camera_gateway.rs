#![forbid(unsafe_op_in_unsafe_fn)]

use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use abi_stable::std_types::{RResult, RString};

use newengine_camera::{
    CameraChannel, CameraChannelState, CameraViewport, RuntimeNavController, RuntimeNavMode,
};
use newengine_camera_api::{
    CameraServiceInfo, CameraViewCommand, CameraViewCommandRequest, CameraViewCommandResponse,
    CameraViewMode, CAMERA_BACKEND_CAPABILITY_ID, ENGINE_CAMERA_SERVICE_ID,
};
use newengine_camera_contracts::CameraFrameSnapshot;
use newengine_camera_runtime::{
    camera_frame_snapshot_for_view, cursor_state_for_nav, step_camera_nav, BoundsSphere as CamBoundsSphere,
    CameraManagerResource, CameraNavFrameRequest, CameraNavInput, CameraNavParams,
    CameraRuntimeReport, CameraRuntimeService, CameraRuntimeServiceConfig, CameraRuntimeWorldState,
    CameraTransitionPhase as RuntimeCameraTransitionPhase,
};
use newengine_core::host_events::CursorState;
use newengine_core::render::{
    PostFxFrameParams, ViewDepthOfFieldFrameParams, ViewMotionBlurFrameParams,
    ViewPostFxFrameParams,
};
use newengine_ecs::{EntityId, World};
use newengine_input_actions_api::CameraViewRequest;
use newengine_math::{Mat4, Vec2, Vec3};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    decode_json_payload, engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use newengine_transform::Transform;

use crate::gameplay::{
    capture_runtime_world_snapshot, emit_player_event, first_player, is_player_controller_enabled,
    restore_runtime_world_snapshot, sync_player_view_listeners, FpsDemoRules, GameRunMode,
    PlayerEventKind, RuntimeWorldSnapshot,
};
use crate::engine_bounds::EngineBoundsSnap;
use crate::viewport_bridge::ViewportBridge;


const CAMERA_GATEWAY_OWNER: &str = "newengine-engine-runtime.camera-gateway";
static CAMERA_GATEWAY_REGISTERED: AtomicBool = AtomicBool::new(false);

#[path = "camera_gateway_helpers.rs"]
mod camera_gateway_helpers;
use self::camera_gateway_helpers::{
    camera_nav_input, camera_report_snapshot, camera_runtime_service_config,
    apply_runtime_input, sanitize_camera_dt, view_postfx_from_camera_snapshot,
};
pub use self::camera_gateway_helpers::{
    apply_view_postfx, CameraRuntimeOverlayReport, CameraTransitionOverlayReport,
    CameraTransitionPhase,
};

fn camera_gateway_service(state: Arc<Mutex<CameraGatewayState>>) -> newengine_plugin_api::ServiceV1Dyn<'static> {
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
        .get_json(newengine_camera_api::CAMERA_SERVICE_METHOD_SNAPSHOT_JSON_V1, |state| {
            state.last_snapshot.unwrap_or_default()
        })
        .blob(newengine_camera_api::CAMERA_SERVICE_METHOD_INVOKE, |state, payload| {
            invoke_camera_gateway(state, payload)
        })
        .blob(newengine_camera_api::CAMERA_SERVICE_METHOD_VIEW_SET_JSON_V1, |state, payload| {
            apply_camera_view_command(state, payload)
        })
        .get_json(newengine_camera_api::CAMERA_SERVICE_METHOD_VIEW_NEXT_JSON_V1, |state| {
            let active_view = state.set_view_command(CameraViewCommand::Next);
            CameraViewCommandResponse { active_view }
        })
        .blob(newengine_camera_api::CAMERA_SERVICE_METHOD_SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

fn invoke_camera_gateway(
    state: &mut CameraGatewayState,
    payload: Blob,
) -> RResult<Blob, RString> {
    if payload.as_slice().is_empty() {
        let snapshot = state.last_snapshot.unwrap_or_default();
        return ok_json(&snapshot);
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
    ok_json(&CameraViewCommandResponse { active_view })
}

fn register_camera_gateway_service_best_effort(state: Arc<Mutex<CameraGatewayState>>) {
    // Early runtime bootstrap must not resolve `engine.camera` through the
    // gateway registry and must not emit legacy routed logs. The in-process
    // stargazer provider is idempotently installed once; later providers can
    // still override by normal gateway priority rules.
    if CAMERA_GATEWAY_REGISTERED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    let service = camera_gateway_service(state);
    if register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_CAMERA_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Camera,
        provider_service: ENGINE_CAMERA_SERVICE_ID,
        provider_route: "engine.camera.stargazer",
        capability: CAMERA_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: CAMERA_GATEWAY_OWNER,
        service,
    })
    .is_err()
    {
        CAMERA_GATEWAY_REGISTERED.store(false, Ordering::Release);
    }
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
        register_camera_gateway_service_best_effort(Arc::clone(&state));
        Self { state }
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
        let mut nav_input = camera_nav_input(input, play_mode);
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
        let gate_blocked = play_mode.is_runtime() && !world_playable;

        let suppress_game_nav = {
            let manager = world
                .resource_mut::<CameraManagerResource>()
                .expect("camera manager resource inserted");
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
        apply_runtime_input(world, input, effective_play_mode, service_config);

        let params = CameraNavParams {
            dt: camera_dt,
            viewport: CameraViewport::from_size(vp_w, vp_h),
            channel: CameraChannelState::dominant(if effective_play_mode.is_runtime() {
                CameraChannel::Gameplay
            } else {
                CameraChannel::Runtime
            }),
            bounds: CamBoundsSphere { center: bounds.center, radius: bounds.radius },
            selection_bounds: selection_bounds.map(|b| CamBoundsSphere {
                center: b.center,
                radius: b.radius,
            }),
        };

        let frame_req = CameraNavFrameRequest {
            seq: viewport.read_frame_request(),
            all: viewport.read_frame_all(),
        };

        if suppress_game_nav || effective_play_mode.wants_direct_player_control() || nav_input.navigation_gated {
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

        let (snapshot, report) = if let Some(manager) = world.resource_mut::<CameraManagerResource>() {
            manager.sync_runtime_nav_mode_from_controller(out.controller.mode);
            manager.set_last_cursor(out.cursor);
            let frame = manager.resolve_camera_frame(out.frame, dt);
            let effects = manager.last_post_effects().unwrap_or_default();
            (camera_frame_snapshot_for_view(frame, effects, manager.active_view_mode()), Some(camera_report_snapshot(manager.report())))
        } else {
            (camera_frame_snapshot_for_view(out.frame, Default::default(), active_view), None)
        };

        state.last_snapshot = Some(snapshot);
        let view = EngineViewFrame::from_camera_snapshot(snapshot);
        let cursor = if effective_play_mode.wants_direct_player_control() && input.active && !input.camera_navigation_gated {
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
    runtime_session: Option<RuntimeWorldSnapshot>,
    last_snapshot: Option<CameraFrameSnapshot>,
    active_view: CameraViewMode,
}


impl Default for CameraGatewayState {
    #[inline]
    fn default() -> Self {
        Self {
            nav: newengine_camera_runtime::CameraNavState::default(),
            last_play_mode: GameRunMode::Staging,
            play_session: None,
            runtime_session: None,
            last_snapshot: None,
            active_view: CameraViewMode::FirstPerson,
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

        if !self.last_play_mode.is_runtime() && effective_play_mode.is_runtime() {
            self.runtime_session = Some(capture_runtime_world_snapshot(world));
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
            self.play_session = Some(CameraPlaySessionSnapshot { cam_id, rig, transform });
        }

        if self.last_play_mode.is_runtime() && !effective_play_mode.is_runtime() {
            if let Some(snapshot) = self.runtime_session.take() {
                restore_runtime_world_snapshot(world, snapshot);
            }
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

#[derive(Clone, Copy, Debug, Default)]
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

