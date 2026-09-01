use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

use newengine_core::render::{require_render_api, BeginFrameDesc, Extent2D, SceneLaunchStatus};
use newengine_core::{EngineResult, ModuleCtx};
use newengine_gameplay_world_runtime::gameplay::WorldClearColor;
use newengine_ui_api::{
    UiLayerCompositionPlan, UiLayerDomain, UiLayerDrawPacketSet, UiPresentationFlowState,
    UiRuntimeDebugOverlayTelemetry, UiScreenProfile, UiScreenProfileState, UiViewportSlot,
};

use super::super::controller::RuntimeRenderController;
use super::super::error_policy::{
    is_backend_device_lost_error, is_transient_shader_pipeline_error,
};
use super::frame_types::{PlayableFrameOutcome, RenderFrameScope};

const ASSET_PREVIEW_EDITOR_CLEAR_COLOR: [f32; 4] = [0.16, 0.17, 0.19, 1.0];

#[path = "render_entry/policy.rs"]
mod policy;
#[path = "render_entry/runtime.rs"]
mod runtime;
#[path = "render_entry/trace.rs"]
mod trace;
#[path = "render_entry/ui_only.rs"]
mod ui_only;

#[cfg(test)]
use policy::parse_runtime_debug_overlay_setting;
use policy::{panic_payload_message, resolve_viewport_clear_color, runtime_debug_overlay_enabled};

#[cfg(test)]
include!("render_entry/tests.rs");
