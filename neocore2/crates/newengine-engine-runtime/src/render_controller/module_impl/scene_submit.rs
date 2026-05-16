#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{RenderApi, RenderTargetId};
use newengine_core::EngineResult;
use newengine_scene::Scene;
use newengine_ui::draw::UiDrawList;

use super::frame_orchestrator::RenderFrameOrchestrator;
use super::frame_types::{PlayableFrameOutcome, RenderFrameScope, WorldFrameState};
use super::super::controller::RuntimeRenderController;
use crate::gameplay::GameRunMode;

impl RuntimeRenderController {
    pub(super) fn submit_scene_viewport_frame(
        &mut self,
        r: &mut dyn RenderApi,
        scene: &Scene,
        plugin_snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
        ui: Option<&UiDrawList>,
        requested_play_mode: GameRunMode,
        rt: Option<RenderTargetId>,
        scope: RenderFrameScope,
        world_frame: &WorldFrameState,
    ) -> EngineResult<PlayableFrameOutcome> {
        RenderFrameOrchestrator::submit_scene_viewport_frame(
            self,
            r,
            scene,
            plugin_snapshot,
            ui,
            requested_play_mode,
            rt,
            scope,
            world_frame,
        )
    }
}
