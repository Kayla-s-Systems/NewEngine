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
        let play_mode =
            editor_viewport_play_mode(ctx).unwrap_or_else(|| self.bridges.scene.play_mode());
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

pub(super) fn editor_viewport_runtime_mode<E: Send + 'static>(
    ctx: &ModuleCtx<'_, E>,
) -> Option<UiEditorRuntimeMode> {
    let profile = ctx
        .resources()
        .get::<UiScreenProfileState>()
        .map(|state| state.descriptor.profile)
        .unwrap_or_default();
    if profile != UiScreenProfile::Editor {
        return None;
    }
    Some(
        match ctx
            .resources()
            .get::<RuntimeSessionState>()
            .and_then(|state| state.mode)
        {
            Some(RuntimeSessionMode::Simulate) => UiEditorRuntimeMode::Simulate,
            Some(RuntimeSessionMode::Play) => UiEditorRuntimeMode::Play,
            None => UiEditorRuntimeMode::Edit,
        },
    )
}

pub(super) fn editor_viewport_play_mode<E: Send + 'static>(
    ctx: &ModuleCtx<'_, E>,
) -> Option<crate::gameplay::GameRunMode> {
    let mode = editor_viewport_runtime_mode(ctx)?;
    Some(match mode {
        UiEditorRuntimeMode::Edit => crate::gameplay::GameRunMode::Staging,
        UiEditorRuntimeMode::Simulate => crate::gameplay::GameRunMode::Simulate,
        UiEditorRuntimeMode::Play => {
            if ctx
                .resources()
                .get::<RuntimeSessionState>()
                .is_some_and(|state| state.is_possessed())
            {
                crate::gameplay::GameRunMode::Play
            } else {
                crate::gameplay::GameRunMode::Simulate
            }
        }
    })
}
