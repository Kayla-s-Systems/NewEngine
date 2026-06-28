use newengine_render_api::{RenderGraphPassDesc, RenderGraphResourceId};

use crate::{RenderFramePlan, RenderPhaseDesc, StandardRenderPhase};

use super::{FrameGraphBuilder, RG_SCENE_HDR_COLOR, RG_SURFACE_COLOR, RG_VIEWPORT_COLOR};

impl FrameGraphBuilder {
    #[inline]
    pub fn submit(mut self) -> RenderFramePlan {
        self.finalize_surface_output();
        self.phases
            .push(RenderPhaseDesc::standard(StandardRenderPhase::EndFrame));
        let mut plan = RenderFramePlan::new(self.graph);
        plan.phases = self.phases;
        plan.draw_lists = self.draw_lists;
        plan.execution_mode = self.execution_mode;
        plan
    }

    #[inline]
    pub(super) fn viewport_color_resource(&mut self) -> RenderGraphResourceId {
        if self.target.hdr_scene_enabled {
            return RG_SCENE_HDR_COLOR;
        }
        if self.target.viewport_is_surface {
            RG_SURFACE_COLOR
        } else {
            RG_VIEWPORT_COLOR
        }
    }

    pub(super) fn add_phase_pass(
        &mut self,
        phase: StandardRenderPhase,
        build: impl FnOnce(RenderGraphPassDesc) -> RenderGraphPassDesc,
    ) {
        let Some(kind) = phase.pass_kind() else {
            return;
        };
        let id = phase.stable_pass_id().unwrap_or_else(|| {
            let id = self.next_custom_pass;
            self.next_custom_pass = self.next_custom_pass.saturating_add(1);
            newengine_render_api::RenderGraphPassId(id)
        });
        let pass = build(RenderGraphPassDesc::new(id, phase.label(), kind));
        self.graph.passes.push(pass);
        self.phases.push(RenderPhaseDesc::standard(phase));
    }

    #[inline]
    pub(super) fn has_resource(&self, id: RenderGraphResourceId) -> bool {
        self.graph
            .resources
            .iter()
            .any(|resource| resource.id == id)
    }
}
