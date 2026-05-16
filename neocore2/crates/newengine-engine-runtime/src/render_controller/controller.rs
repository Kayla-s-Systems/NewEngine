#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use crate::plugin_manager::PluginManagerBridge;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

use super::state::{
    RenderBridgeState, RenderDiagnosticsRuntimeState, RenderFrameRuntimeState, RenderGpuSceneState,
    RenderShadowRuntimeState, RenderViewportState,
};

/// Engine-side render composition root.
///
/// This type is intentionally a composition root, not a renderer backend. It owns
/// runtime-facing state, delegates frame orchestration to module_impl, and submits
/// a typed RenderFrameEnvelope into the backend RenderApi adapter.
pub struct RuntimeRenderController {
    pub(super) bridges: RenderBridgeState,
    pub(super) viewport: RenderViewportState,
    pub(super) shadows: RenderShadowRuntimeState,
    pub(super) gpu: RenderGpuSceneState,
    pub(super) frame: RenderFrameRuntimeState,
    pub(super) diagnostics: RenderDiagnosticsRuntimeState,
}

impl RuntimeRenderController {
    pub(super) fn disable_viewport_pass(
        &mut self,
        phase: &'static str,
        error: impl std::fmt::Display,
    ) {
        let message = error.to_string();
        if !self.viewport.pass_disabled {
            log::error!(
                "render controller: viewport GPU pass disabled phase='{}' err='{}'",
                phase,
                message
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: viewport GPU pass disabled phase='{}' err='{}'",
                phase, message
            ));
        }
        self.viewport.pass_disabled = true;
    }

    #[inline]
    pub fn new(
        viewport_bridge: Arc<ViewportBridge>,
        plugins_bridge: Arc<PluginManagerBridge>,
        scene_bridge: Arc<SceneBridge>,
    ) -> Self {
        Self {
            bridges: RenderBridgeState::new(viewport_bridge, plugins_bridge, scene_bridge),
            viewport: RenderViewportState::new(),
            shadows: RenderShadowRuntimeState::new(),
            gpu: RenderGpuSceneState::new(),
            frame: RenderFrameRuntimeState::new(),
            diagnostics: RenderDiagnosticsRuntimeState::new(),
        }
    }
}
