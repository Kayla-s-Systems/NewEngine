#![forbid(unsafe_op_in_unsafe_fn)]

use parking_lot::Mutex;
use std::sync::Arc;

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
use newengine_input_bindings::CameraViewRequest;
use newengine_math::{Mat4, Vec2, Vec3};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    decode_json_payload, engine_owned_service_description, ok_empty_blob, ok_json,
    register_engine_owned_gateway_service, EngineOwnedGatewayDecl, JsonServiceRouter,
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

fn camera_gateway_service(state: Arc<Mutex<CameraGatewayState>>) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = CameraServiceInfo::default();
    let description = engine_owned_service_description(
        ENGINE_CAMERA_SERVICE_ID,
        CAMERA_GATEWAY_OWNER,
        CAMERA_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .gateway("engine-owned engine.camera in-process bridge");

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
    if newengine_plugin_host::has_service(ENGINE_CAMERA_SERVICE_ID) {
        return;
    }
    let service = camera_gateway_service(state);
    match register_engine_owned_gateway_service(EngineOwnedGatewayDecl {
        gateway: ENGINE_CAMERA_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Camera,
        provider_service: ENGINE_CAMERA_SERVICE_ID,
        capability: CAMERA_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: CAMERA_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => log::info!(
            "camera gateway: engine-owned route registered id='{}' capability='{}'",
            ENGINE_CAMERA_SERVICE_ID,
            CAMERA_BACKEND_CAPABILITY_ID
        ),
        Err(e) => log::warn!(
            "camera gateway: registration skipped id='{}' err='{}'",
            ENGINE_CAMERA_SERVICE_ID,
            e
        ),
    }
}

/// Engine-owned camera gateway bridge.
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
            manager.advance(dt);
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
            dt,
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

        if suppress_game_nav || effective_play_mode.wants_direct_player_control() {
            nav_input.active = false;
            nav_input.look_drag = false;
            nav_input.pan_drag = false;
            nav_input.fly_rmb = false;
            nav_input.move_mask = 0;
            nav_input.wheel_y = 0.0;
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
        let cursor = if effective_play_mode.wants_direct_player_control() && input.active {
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
    pub forward_ws: Vec3,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub aspect: f32,
}

impl EngineViewFrame {
    #[inline]
    fn from_camera_snapshot(snapshot: CameraFrameSnapshot) -> Self {
        Self {
            view: mat4_from_cols(snapshot.view_cols),
            projection: mat4_from_cols(snapshot.projection_cols),
            view_projection: mat4_from_cols(snapshot.view_projection_cols),
            inverse_view: mat4_from_cols(snapshot.inverse_view_cols),
            position_ws: arr_vec3(snapshot.position_ws),
            forward_ws: arr_vec3(snapshot.forward_ws),
            viewport_width: snapshot.viewport.width,
            viewport_height: snapshot.viewport.height,
            aspect: snapshot.viewport.aspect,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraRuntimeOverlayReport {
    pub active_director: String,
    pub active_mode: String,
    pub active_view_mode: String,
    pub target_entity: Option<EntityId>,
    pub transition: CameraTransitionOverlayReport,
    pub input_context: String,
    pub gate_blocked: bool,
    pub frame_blend_active: bool,
    pub frame_blend_alpha: f32,
    pub dominant_director: Option<String>,
    pub rendered_director_count: usize,
    pub director_lock_input: bool,
    pub pending_event_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraTransitionPhase {
    Idle,
    Pending,
    Blending,
}

#[derive(Clone, Debug)]
pub struct CameraTransitionOverlayReport {
    pub phase: CameraTransitionPhase,
    pub elapsed_sec: f32,
}

#[inline]
pub fn apply_view_postfx(mut params: PostFxFrameParams, view: ViewPostFxFrameParams) -> PostFxFrameParams {
    params.display.exposure *= 2.0f32.powf(view.exposure_bias);
    params.view = view;
    params
}

#[inline]
fn camera_runtime_service_config(world: &World, active_view: CameraViewMode) -> CameraRuntimeServiceConfig {
    let mut config = CameraRuntimeServiceConfig::default();
    if let Some(rules) = world.resource::<FpsDemoRules>() {
        config.first_person_eye_height = rules.player.camera_eye_height;
        config.sprint_multiplier = rules.player.sprint_multiplier;
    }
    config.runner = match active_view {
        CameraViewMode::FirstPerson => newengine_camera_runtime::GameplayCameraRunnerKind::FirstPerson,
        CameraViewMode::ThirdPersonFollow => newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonFollow,
        CameraViewMode::ThirdPersonAim => newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonAim,
    };
    config
}

fn apply_runtime_input(
    world: &mut World,
    input: CameraGatewayInput,
    effective_play_mode: GameRunMode,
    service_config: CameraRuntimeServiceConfig,
) {
    let Some(player) = first_player(world) else {
        return;
    };
    if effective_play_mode.wants_direct_player_control() && is_player_controller_enabled(world, player) {
        CameraRuntimeService::apply_player_input(
            world,
            player,
            input.move_mask,
            Vec2::new(-input.dx_px, -input.dy_px),
            input.active,
            service_config.sprint_multiplier,
        );
        emit_player_event(world, player, PlayerEventKind::InputApplied, "local input sampled");
    } else {
        CameraRuntimeService::clear_player_input(world, player);
    }
}

#[inline]
fn camera_nav_input(input: CameraGatewayInput, play_mode: GameRunMode) -> CameraNavInput {
    let mut nav_input = CameraNavInput {
        dx_px: input.dx_px,
        dy_px: input.dy_px,
        wheel_y: input.wheel_y,
        active: input.active,
        look_drag: input.look_drag,
        pan_drag: input.pan_drag,
        ui_busy: input.ui_busy,
        fly_rmb: input.fly_rmb,
        move_mask: input.move_mask,
        speed_scalar: input.speed_scalar,
    };
    if play_mode.wants_direct_player_control() {
        nav_input.wheel_y = 0.0;
        nav_input.pan_drag = false;
    }
    nav_input
}

#[inline]
fn view_postfx_from_camera_snapshot(snapshot: CameraFrameSnapshot) -> ViewPostFxFrameParams {
    let postfx = snapshot.postfx;
    ViewPostFxFrameParams {
        dof: ViewDepthOfFieldFrameParams {
            near_start: postfx.dof.near_start,
            near_end: postfx.dof.near_end,
            far_start: postfx.dof.far_start,
            far_end: postfx.dof.far_end,
            blend_level: postfx.dof.blend_level,
            high_quality: postfx.dof.high_quality,
        },
        motion_blur: ViewMotionBlurFrameParams {
            strength: postfx.motion_blur.strength,
            decay_rate: postfx.motion_blur.decay_rate,
        },
        shake_amplitude: postfx.shake_amplitude,
        exposure_bias: postfx.exposure_bias,
        jitter_px: postfx.jitter_px,
    }
}

#[inline]
fn camera_report_snapshot(report: CameraRuntimeReport) -> CameraRuntimeOverlayReport {
    CameraRuntimeOverlayReport {
        active_director: format!("{:?}", report.active_director),
        active_mode: format!("{:?}", report.active_mode),
        active_view_mode: format!("{:?}", report.view_mode),
        target_entity: report.target_entity,
        transition: CameraTransitionOverlayReport {
            phase: match report.transition.phase {
                RuntimeCameraTransitionPhase::Idle => CameraTransitionPhase::Idle,
                RuntimeCameraTransitionPhase::Pending => CameraTransitionPhase::Pending,
                RuntimeCameraTransitionPhase::Blending => CameraTransitionPhase::Blending,
            },
            elapsed_sec: report.transition.elapsed_sec,
        },
        input_context: format!("{:?}", report.input_context),
        gate_blocked: report.gate_blocked,
        frame_blend_active: report.frame_blend_active,
        frame_blend_alpha: report.frame_blend_alpha,
        dominant_director: report.dominant_director.map(|it| format!("{:?}", it)),
        rendered_director_count: report.rendered_director_count,
        director_lock_input: report.director_lock_input,
        pending_event_count: report.pending_event_count,
    }
}

#[inline]
fn mat4_from_cols(cols: [[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array(&[
        cols[0][0], cols[0][1], cols[0][2], cols[0][3],
        cols[1][0], cols[1][1], cols[1][2], cols[1][3],
        cols[2][0], cols[2][1], cols[2][2], cols[2][3],
        cols[3][0], cols[3][1], cols[3][2], cols[3][3],
    ])
}

#[inline]
fn arr_vec3(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}
