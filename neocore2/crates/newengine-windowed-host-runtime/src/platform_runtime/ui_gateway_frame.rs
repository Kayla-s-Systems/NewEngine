#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use newengine_core::{EngineResult, StableServiceCall};
use newengine_system_contracts::{ScreenOverlayStatus, ScreenOverlayStatusKind};
use newengine_system_runtime::loading_surface_projection;
use newengine_ui::UiProviderBinding;
use newengine_ui_api::{
    decode_ui_frame_response_bin, encode_ui_frame_request_bin, reserved, UiActionDispatch,
    UiComponentNode, UiDispatchActionRequest, UiDispatchInputRequest, UiDrawList,
    UiEventDispatchFrame, UiFrameRequest, UiFrameResponse, UiImagePaintCommand, UiInputFrame,
    UiNodeTreeRequest, UiPaintCommand, UiPaintNodeRef, UiRuntimeDebugOverlayTelemetry,
    UiStatePatch, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceRequest, UiSurfaceStyle,
    UiSurfaceVisibilityRequest, UiTexId, ENGINE_UI_SERVICE_ID, UI_COMPONENT_PANEL,
    UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1, UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
    UI_SERVICE_METHOD_DISPATCH_ACTION_V1, UI_SERVICE_METHOD_DISPATCH_INPUT_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1, UI_SERVICE_METHOD_DRAW_FRAME_V1,
    UI_SERVICE_METHOD_SET_SURFACE_VISIBLE_V1, UI_SERVICE_METHOD_SURFACE_NODE_V1,
    UI_SERVICE_METHOD_UNMOUNT_SURFACE_V1, UI_SURFACE_ENGINE_ERROR_MODAL, UI_SURFACE_ENGINE_LOADING,
    UI_SURFACE_RUNTIME_DEBUG_OVERLAY, UI_THEME_NORTHSTAR_DEFAULT,
};
use serde::Deserialize;

static TRY_BINARY_UI_FRAME: AtomicBool = AtomicBool::new(true);

static UI_DRAW_FRAME_BIN_CALL: OnceLock<StableServiceCall> = OnceLock::new();
static UI_DRAW_FRAME_JSON_CALL: OnceLock<StableServiceCall> = OnceLock::new();
static UI_DISPATCH_INPUT_CALL: OnceLock<StableServiceCall> = OnceLock::new();
static UI_APPLY_STATE_PATCH_CALL: OnceLock<StableServiceCall> = OnceLock::new();
static UI_SURFACE_NODE_CALL: OnceLock<StableServiceCall> = OnceLock::new();
static UI_APPLY_NODE_REQUEST_CALL: OnceLock<StableServiceCall> = OnceLock::new();
static UI_UNMOUNT_SURFACE_CALL: OnceLock<StableServiceCall> = OnceLock::new();
static UI_SET_SURFACE_VISIBLE_CALL: OnceLock<StableServiceCall> = OnceLock::new();

#[inline]
fn stable_ui_call(
    slot: &'static OnceLock<StableServiceCall>,
    method: &'static str,
) -> &'static StableServiceCall {
    slot.get_or_init(|| StableServiceCall::new(ENGINE_UI_SERVICE_ID, method))
}

#[inline]
fn ui_draw_frame_bin_call() -> &'static StableServiceCall {
    stable_ui_call(&UI_DRAW_FRAME_BIN_CALL, UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1)
}

#[inline]
fn ui_draw_frame_json_call() -> &'static StableServiceCall {
    stable_ui_call(&UI_DRAW_FRAME_JSON_CALL, UI_SERVICE_METHOD_DRAW_FRAME_V1)
}

#[inline]
fn ui_dispatch_input_call() -> &'static StableServiceCall {
    stable_ui_call(&UI_DISPATCH_INPUT_CALL, UI_SERVICE_METHOD_DISPATCH_INPUT_V1)
}

#[inline]
fn ui_apply_state_patch_call() -> &'static StableServiceCall {
    stable_ui_call(
        &UI_APPLY_STATE_PATCH_CALL,
        UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
    )
}

#[inline]
fn ui_surface_node_call() -> &'static StableServiceCall {
    stable_ui_call(&UI_SURFACE_NODE_CALL, UI_SERVICE_METHOD_SURFACE_NODE_V1)
}

#[inline]
fn ui_apply_node_request_call() -> &'static StableServiceCall {
    stable_ui_call(
        &UI_APPLY_NODE_REQUEST_CALL,
        UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1,
    )
}

#[inline]
fn ui_unmount_surface_call() -> &'static StableServiceCall {
    stable_ui_call(
        &UI_UNMOUNT_SURFACE_CALL,
        UI_SERVICE_METHOD_UNMOUNT_SURFACE_V1,
    )
}

#[inline]
fn ui_set_surface_visible_call() -> &'static StableServiceCall {
    stable_ui_call(
        &UI_SET_SURFACE_VISIBLE_CALL,
        UI_SERVICE_METHOD_SET_SURFACE_VISIBLE_V1,
    )
}

#[derive(Clone, Debug)]
pub(crate) struct UiGatewayFramePolicy {
    pub binary_frame_required: bool,
    pub json_fallback_allowed: bool,
}

impl Default for UiGatewayFramePolicy {
    #[inline]
    fn default() -> Self {
        Self {
            binary_frame_required: false,
            json_fallback_allowed: true,
        }
    }
}

impl UiGatewayFramePolicy {
    pub(crate) fn from_startup_config(startup: Option<&newengine_core::StartupConfig>) -> Self {
        let mut policy = Self::default();
        let Some(startup) = startup else {
            return policy;
        };
        let Some(value) = startup.plugins.get(ENGINE_UI_SERVICE_ID) else {
            return policy;
        };
        let Ok(config) = serde_json::from_value::<UiGatewayPluginConfig>(value.clone()) else {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: invalid engine.ui config shape; using default frame policy"
            );
            return policy;
        };

        policy.binary_frame_required = config
            .require_binary_frame
            .or(config.binary_frame_required)
            .unwrap_or(policy.binary_frame_required);
        policy.json_fallback_allowed = config
            .allow_json_fallback
            .unwrap_or(policy.json_fallback_allowed);

        if policy.binary_frame_required {
            policy.json_fallback_allowed = false;
        }

        newengine_ulog_api::ulog::info!(
            "ui gateway: frame policy binary_required={} json_fallback_allowed={} source='plugins.engine.ui'",
            policy.binary_frame_required,
            policy.json_fallback_allowed,
        );
        policy
    }

    pub(crate) fn handle_binary_error(&self, err: String) -> EngineResult<bool> {
        if self.binary_frame_required || !self.json_fallback_allowed {
            newengine_ulog_api::ulog::error!(
                "ui gateway: binary draw-frame path required by engine.ui policy; JSON fallback remains disabled err='{}'",
                err
            );
            return Ok(false);
        }

        if TRY_BINARY_UI_FRAME.swap(false, Ordering::Relaxed) {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: binary draw-frame path unavailable; falling back to JSON control path err='{}'",
                err
            );
        }
        Ok(true)
    }
}

#[derive(Debug, Deserialize)]
struct UiGatewayPluginConfig {
    #[serde(default)]
    allow_json_fallback: Option<bool>,
    #[serde(default)]
    require_binary_frame: Option<bool>,
    #[serde(default)]
    binary_frame_required: Option<bool>,
}

// Routes the current platform/input snapshot through the active `engine.ui` dispatcher.
//
// This is the canonical input spine for retained UI: platform/runtime collects a
// provider-neutral `UiInputFrame`, the selected UI provider owns hit-testing,
// focus, hover, pointer capture and action emission, and product modules consume
// `UiEventDispatchFrame` instead of calculating private rectangles.

#[path = "ui_gateway_frame_parts/draw_list.rs"]
mod draw_list;
#[path = "ui_gateway_frame_parts/input_dispatch.rs"]
mod input_dispatch;
#[path = "ui_gateway_frame_parts/loading_overlay.rs"]
mod loading_overlay;
#[path = "ui_gateway_frame_parts/publish.rs"]
mod publish;

pub(crate) use self::draw_list::{
    animate_loading_draw_list, loading_animation_now_ms, request_ui_draw_list,
};
pub(crate) use self::input_dispatch::dispatch_input_frame;
pub(crate) use self::loading_overlay::{publish_loading_overlay, publish_loading_overlay_inactive};
pub(crate) use self::publish::{
    publish_debug_overlay_telemetry, publish_node_tree_request, publish_surface_node,
    set_surface_visible,
};
