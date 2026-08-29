use super::*;

impl RuntimeRenderController {
    pub(super) fn read_viewport_frame_input<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        ui_layers: UiLayerDrawPacketSet,
        primary_ui_domain: UiLayerDomain,
        scope: RenderFrameScope,
    ) -> ViewportFrameInput {
        let surface_input = ctx
            .resources()
            .get::<newengine_ui_api::UiInputFrame>()
            .cloned();
        let play_mode = self.bridges.scene.play_mode();
        let mut input = if scope.direct_surface_viewport {
            ViewportInputSnap::read_direct_surface(surface_input.as_ref())
        } else {
            let mut input = ViewportInputSnap::read(&self.bridges.viewport);
            input.merge_semantic_actions_from_surface(
                surface_input.as_ref(),
                play_mode.wants_direct_player_control(),
            );
            input
        };
        {
            let mut carrier = input.action_carrier();
            self.frame.input_systems.observe_frame(
                self.frame.frame_index,
                surface_input.as_ref(),
                &mut carrier,
            );
        }
        if play_mode.wants_direct_player_control() {
            input.apply_gameplay_input_handoff(&self.runtime_profile().input);
        }
        ViewportFrameInput {
            ui_layers,
            primary_ui_domain,
            input,
            surface_input,
            play_mode,
        }
    }
}

pub(super) fn is_game_screen_profile<E: Send + 'static>(ctx: &ModuleCtx<'_, E>) -> bool {
    ctx.resources()
        .get::<UiScreenProfileState>()
        .map(|state| state.descriptor.profile == UiScreenProfile::Game)
        .unwrap_or(true)
}
