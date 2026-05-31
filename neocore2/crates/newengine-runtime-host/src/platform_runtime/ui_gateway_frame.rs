#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use newengine_core::EngineResult;
use newengine_system_contracts::ScreenOverlayStatus;
use newengine_system_runtime::loading_surface_projection;
use newengine_ui::UiProviderBinding;
use newengine_ui_api::{
    decode_ui_frame_response_bin, encode_ui_frame_request_bin, UiActionDispatch, UiComponentNode,
    UiDispatchActionRequest, UiDispatchInputRequest, UiDrawList, UiEventDispatchFrame,
    UiFrameRequest, UiFrameResponse, UiInputFrame, UiNodeRequestAck, UiNodeTone, UiNodeTreeRequest,
    UiRuntimeDebugOverlayTelemetry, UiStatePatch, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle,
    ENGINE_UI_SERVICE_ID, UI_COMPONENT_PANEL, UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1,
    UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1, UI_SERVICE_METHOD_DISPATCH_ACTION_V1,
    UI_SERVICE_METHOD_DISPATCH_INPUT_V1, UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_V1, UI_SERVICE_METHOD_SURFACE_NODE_V1, UI_SURFACE_ENGINE_LOADING,
    UI_SURFACE_RUNTIME_DEBUG_OVERLAY, UI_THEME_NORTHSTAR_DEFAULT,
};

static TRY_BINARY_UI_FRAME: AtomicBool = AtomicBool::new(true);


/// Routes the current platform/input snapshot through the active `engine.ui` dispatcher.
///
/// This is the canonical input spine for retained UI: platform/runtime collects a
/// provider-neutral `UiInputFrame`, the selected UI provider owns hit-testing,
/// focus, hover, pointer capture and action emission, and product modules consume
/// `UiEventDispatchFrame` instead of calculating private rectangles.

// Same-scope ownership split for UI gateway frame helpers.
include!("ui_gateway_frame_parts/input_dispatch.rs");
include!("ui_gateway_frame_parts/draw_list.rs");
include!("ui_gateway_frame_parts/loading_overlay.rs");
include!("ui_gateway_frame_parts/publish.rs");
