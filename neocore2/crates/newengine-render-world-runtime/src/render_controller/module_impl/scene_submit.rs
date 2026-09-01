#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{RenderApi, RenderTargetId};
use newengine_core::{EngineResult, ThreadPoolHandle};
use newengine_scene::Scene;
use newengine_ui_api::UiLayerDrawPacketSet;

use super::super::controller::RuntimeRenderController;
use super::frame_orchestrator::RenderFrameOrchestrator;
use super::frame_types::{PlayableFrameOutcome, RenderFrameScope, WorldFrameState};
use newengine_gameplay_world_runtime::gameplay::GameRunMode;

impl RuntimeRenderController {
    pub(super) fn submit_scene_viewport_frame(
        &mut self,
        r: &mut dyn RenderApi,
        scene: &Scene,
        plugin_snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
        ui_layers: UiLayerDrawPacketSet,
        requested_play_mode: GameRunMode,
        rt: Option<RenderTargetId>,
        scope: RenderFrameScope,
        world_frame: &WorldFrameState,
        thread_pool: Option<&ThreadPoolHandle>,
    ) -> EngineResult<PlayableFrameOutcome> {
        RenderFrameOrchestrator::submit_scene_viewport_frame(
            self,
            r,
            scene,
            plugin_snapshot,
            ui_layers,
            requested_play_mode,
            rt,
            scope,
            world_frame,
            thread_pool,
        )
    }
}
