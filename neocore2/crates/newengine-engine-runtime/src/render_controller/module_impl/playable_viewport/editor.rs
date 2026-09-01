use super::*;

#[inline]
fn editing_tools_available<E: Send + 'static>(ctx: &ModuleCtx<'_, E>) -> bool {
    ctx.resources()
        .get::<newengine_plugin_host::PluginsSnapshot>()
        .is_some_and(|snapshot| {
            snapshot.has_loaded_capability(newengine_plugin_api::CAPABILITY_ID_EDITING_TOOLS)
        })
}

impl RuntimeRenderController {
    pub(super) fn prepare_editor_interaction<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        frame_input: &ViewportFrameInput,
        scope: RenderFrameScope,
    ) -> (bool, bool) {
        let editing_tools_available = editing_tools_available(ctx);
        if editing_tools_available
            && frame_input
                .surface_input
                .as_ref()
                .is_some_and(|input| input.is_key_pressed(newengine_input_api::key_code::F2))
        {
            self.bridges.scene.toggle_in_game_editor();
        } else if !editing_tools_available && self.bridges.scene.in_game_editor_enabled() {
            self.bridges.scene.set_in_game_editor_enabled(false);
        }

        let editor_shift_additive = frame_input.surface_input.as_ref().is_some_and(|input| {
            input.is_key_down(newengine_input_api::key_code::SHIFT_LEFT)
                || input.is_key_down(newengine_input_api::key_code::SHIFT_RIGHT)
        });

        if let Some(dispatch_frame) = ctx
            .resources()
            .get::<newengine_ui_api::UiEventDispatchFrame>()
        {
            if editing_tools_available {
                let _ = self
                    .bridges
                    .scene
                    .apply_in_game_editor_actions(dispatch_frame);
            }

            // Re-read after UI actions because the shell's Exit Editor button may have
            // disabled the mode in this same dispatch frame.
            let active_now = editing_tools_available && self.bridges.scene.in_game_editor_enabled();
            if active_now {
                let _ = self
                    .bridges
                    .scene
                    .apply_editor_selection_actions(dispatch_frame, editor_shift_additive);
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

        let live_editing_active =
            editing_tools_available && self.bridges.scene.in_game_editor_enabled();
        let in_game_editor = live_editing_active;
        self.editor_viewport.set_active(live_editing_active);

        // Publish one canonical cross-runtime activation DTO. The windowed host consumes
        // this on the next screen-profile frame to mount/unmount the full UE-like shell.
        let authoring = self.bridges.scene.authored_project_edit_status();
        ctx.resources_mut()
            .insert(newengine_ui_api::UiInGameEditorState {
                version: 1,
                frame_index: self.frame.frame_index,
                enabled: in_game_editor,
                free_fly: in_game_editor,
                noclip: in_game_editor,
                save_available: editing_tools_available,
                dirty_placements: authoring.dirty_placements,
                pending_creates: authoring.pending_creates,
                pending_deletes: authoring.pending_deletes,
                last_save_succeeded: authoring.last_save_succeeded,
                last_save_message: authoring.last_save_message,
            });

        if live_editing_active {
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
                    let pointer_over_action_chrome = ctx
                        .resources()
                        .get::<newengine_ui_api::UiEventDispatchFrame>()
                        .and_then(|dispatch| dispatch.hovered_node.as_ref())
                        .and_then(|hit| hit.action_id.as_deref())
                        .is_some();
                    if !pointer_over_action_chrome
                        && input.is_mouse_pressed(newengine_input_api::mouse_button::LEFT)
                    {
                        if let Some((mouse_x, mouse_y)) = input.mouse_pos {
                            let inside = slot.contains(mouse_x, mouse_y);
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

        if live_editing_active {
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
                    if control_down && input.is_key_pressed(newengine_input_api::key_code::KEY_S) {
                        match self.bridges.scene.save_authored_project_world() {
                            Ok(count) => newengine_ulog_api::ulog::info!(
                                "in-game editor: Ctrl+S project save complete placements={count}"
                            ),
                            Err(error) => newengine_ulog_api::ulog::error!(
                                "in-game editor: Ctrl+S project save failed err='{}'",
                                error
                            ),
                        }
                    } else if control_down
                        && input.is_key_pressed(newengine_input_api::key_code::KEY_D)
                    {
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

        // The old in-game editor continuously picked through a center reticle. The UE-like
        // shell uses normal viewport click selection, so Editor Mode never steals selection
        // just because the user is flying the camera through the scene.
        (live_editing_active, in_game_editor)
    }
}
