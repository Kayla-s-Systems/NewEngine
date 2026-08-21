use super::input::{editor_viewport_runtime_mode, is_game_screen_profile};
use super::*;

impl RuntimeRenderController {
    pub(super) fn prepare_editor_interaction<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        frame_input: &ViewportFrameInput,
        scope: RenderFrameScope,
    ) -> (bool, bool) {
        let game_profile = is_game_screen_profile(ctx);
        if game_profile
            && frame_input
                .surface_input
                .as_ref()
                .is_some_and(|input| input.is_key_pressed(newengine_input_api::key_code::F2))
        {
            self.bridges.scene.toggle_in_game_editor();
        } else if !game_profile && self.bridges.scene.in_game_editor_enabled() {
            self.bridges.scene.set_in_game_editor_enabled(false);
        }

        let editor_staging_preview =
            editor_viewport_runtime_mode(ctx) == Some(UiEditorRuntimeMode::Edit);
        let editor_shift_additive = frame_input.surface_input.as_ref().is_some_and(|input| {
            input.is_key_down(newengine_input_api::key_code::SHIFT_LEFT)
                || input.is_key_down(newengine_input_api::key_code::SHIFT_RIGHT)
        });

        if let Some(dispatch_frame) = ctx
            .resources()
            .get::<newengine_ui_api::UiEventDispatchFrame>()
        {
            if game_profile {
                let _ = self
                    .bridges
                    .scene
                    .apply_in_game_editor_actions(dispatch_frame);
            }
            let _ = self
                .bridges
                .scene
                .apply_editor_selection_actions(dispatch_frame, editor_shift_additive);
            if editor_staging_preview {
                let _ = self
                    .bridges
                    .scene
                    .apply_editor_actor_actions(dispatch_frame);
            }
            {
                let scene_lock = self.bridges.scene.scene();
                let mut scene = scene_lock.write();
                let _ = self
                    .frame
                    .gameplay_ui
                    .dispatch_actions(scene.world_mut(), dispatch_frame);
            }
        }
        let in_game_editor = game_profile && self.bridges.scene.in_game_editor_enabled();
        self.editor_viewport.set_active(editor_staging_preview);
        if editor_staging_preview {
            self.editor_viewport.sync_state(
                ctx.resources()
                    .get::<newengine_ui_api::UiEditorViewportState>()
                    .cloned()
                    .unwrap_or_default(),
            );
            if scope.vp_w > 0 && scope.vp_h > 0 {
                if let (Some(input), Some(slot)) = (
                    frame_input.surface_input.as_ref(),
                    ctx.resources().get::<UiViewportSlot>(),
                ) {
                    let pointer_over_editor_chrome = ctx
                        .resources()
                        .get::<newengine_ui_api::UiEventDispatchFrame>()
                        .and_then(|dispatch| dispatch.hovered_node.as_ref())
                        .and_then(|hit| hit.action_id.as_deref())
                        .is_some_and(|action| {
                            action.starts_with("editor.viewport.")
                                || action.starts_with("editor.dock.")
                                || action.starts_with("editor.content_drawer.")
                        });
                    if !pointer_over_editor_chrome
                        && input.is_mouse_pressed(newengine_input_api::mouse_button::LEFT)
                    {
                        if let Some((mouse_x, mouse_y)) = input.mouse_pos {
                            let inside = mouse_x >= slot.x_px
                                && mouse_y >= slot.y_px
                                && mouse_x < slot.x_px + slot.w_px
                                && mouse_y < slot.y_px + slot.h_px;
                            if inside && slot.w_px > 1.0 && slot.h_px > 1.0 {
                                let local_x =
                                    ((mouse_x - slot.x_px) / slot.w_px) * scope.vp_w as f32;
                                let local_y =
                                    ((mouse_y - slot.y_px) / slot.h_px) * scope.vp_h as f32;
                                self.frame.pending_pick_additive = input
                                    .is_key_down(newengine_input_api::key_code::SHIFT_LEFT)
                                    || input
                                        .is_key_down(newengine_input_api::key_code::SHIFT_RIGHT);
                                self.bridges.viewport.publish_pick_request(local_x, local_y);
                            }
                        }
                    }
                }
            }
        }
        if editor_staging_preview {
            if let Some(input) = frame_input.surface_input.as_ref() {
                let text_input_active = !input.text.is_empty()
                    || !input.text_edit_ops.is_empty()
                    || !input.ime_preedit.is_empty();
                if !text_input_active {
                    let control_down = input
                        .is_key_down(newengine_input_api::key_code::CONTROL_LEFT)
                        || input.is_key_down(newengine_input_api::key_code::CONTROL_RIGHT);
                    let shift_down = input.is_key_down(newengine_input_api::key_code::SHIFT_LEFT)
                        || input.is_key_down(newengine_input_api::key_code::SHIFT_RIGHT);
                    if control_down && input.is_key_pressed(newengine_input_api::key_code::KEY_D) {
                        let _ = self.bridges.scene.duplicate_selected_actors();
                    }
                    if input.is_key_pressed(newengine_input_api::key_code::DELETE) {
                        let _ = self.bridges.scene.delete_selected_actors();
                    }
                    if !control_down && input.is_key_pressed(newengine_input_api::key_code::KEY_F) {
                        self.bridges.viewport.publish_frame_request(shift_down);
                    }
                }
            }
        }

        if in_game_editor && scope.vp_w > 0 && scope.vp_h > 0 {
            self.bridges.viewport.publish_pick_request(
                (scope.vp_w.saturating_sub(1) as f32) * 0.5,
                (scope.vp_h.saturating_sub(1) as f32) * 0.5,
            );
        }
        (editor_staging_preview, in_game_editor)
    }
}
