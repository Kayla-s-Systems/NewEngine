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

use newengine_bounds::EngineBoundsSnap;
use newengine_gameplay_world_runtime::gameplay::{
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
    camera_runtime_service_config, follow_controller_offset_z, gameplay_target_fov_y,
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

pub fn camera_gateway_route_is_authoritative_in_current_host_context() -> bool {
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

include!("camera_gateway/bridge.rs");
include!("camera_gateway/state.rs");
include!("camera_gateway/frame.rs");
