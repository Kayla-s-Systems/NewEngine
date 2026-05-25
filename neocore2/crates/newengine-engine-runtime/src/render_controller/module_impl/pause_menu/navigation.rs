#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_audio_api::AudioFeedbackKind;
use newengine_ui_api::UiInputFrame;
use newengine_ui_menu_runtime::{MenuHitTestState, MenuRuntimeInput, MenuRuntimeOutput};

use super::super::input::ViewportInputSnap;
use super::*;

impl RenderPauseMenuRuntimeState {
    pub(super) fn process_navigation(
        &mut self,
        surface_input: Option<&UiInputFrame>,
        input: &ViewportInputSnap,
        surface_size_px: [u32; 2],
        frame_index: u64,
    ) {
        let item_count = match self.menu.as_ref() {
            Some(menu) => menu.current_items().len(),
            None => return,
        };
        if item_count == 0 {
            return;
        }

        let hit_test = surface_input.map(|input_frame| MenuHitTestState {
            hovered_index: hovered_item_index(
                input_frame.mouse_pos,
                surface_size_px,
                item_count,
                ease_out_cubic(self.visual_alpha),
            ),
            pointer_primary_pressed: input_frame.is_mouse_pressed(1),
        });

        let Some(menu) = self.menu.as_mut() else { return; };
        let output = menu.handle_input(MenuRuntimeInput {
            nav_x: input.actions.menu_nav[0],
            nav_y: input.actions.menu_nav[1],
            accept: input.actions.menu_accept,
            back: input.actions.menu_back,
            hit_test,
        });

        self.apply_menu_runtime_output(output, frame_index);
    }

    fn apply_menu_runtime_output(&mut self, output: MenuRuntimeOutput, frame_index: u64) {
        if output.selection_changed {
            audio(AudioFeedbackKind::UiMenuNavigate, frame_index);
        }

        for feedback in &output.feedback {
            self.flash_menu_feedback(feedback);
        }

        for dispatch in output.route_dispatches {
            self.dispatch_menu_route(dispatch, frame_index);
        }

        if output.close_requested {
            self.open = false;
            self.awaiting_rebind = None;
            if self.feedback.is_none() {
                self.flash_feedback("Resume", "Returning to gameplay", UiPauseMenuMessageSeverity::Success);
            }
        }
    }
}
