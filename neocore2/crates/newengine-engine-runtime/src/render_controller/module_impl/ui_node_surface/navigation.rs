#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_audio_api::AudioFeedbackKind;
use newengine_ui_api::UiInputFrame;
use newengine_ui_navigation_api::{UiNodeHitTestState, UiNodeNavigationInput, UiNodeNavigationOutput};

use super::super::input::ViewportInputSnap;
use super::*;

impl RenderUiNodeSurfaceState {
    pub(super) fn process_navigation(
        &mut self,
        surface_input: Option<&UiInputFrame>,
        input: &ViewportInputSnap,
        surface_size_px: [u32; 2],
        frame_index: u64,
    ) {
        let item_count = match self.navigation.as_ref() {
            Some(navigation) => navigation.current_items().len(),
            None => return,
        };
        if item_count == 0 {
            return;
        }

        let hit_test = surface_input.map(|input_frame| UiNodeHitTestState {
            hovered_index: hovered_item_index(
                input_frame.mouse_pos,
                surface_size_px,
                item_count,
                ease_out_cubic(self.visual_alpha),
            ),
            pointer_primary_pressed: input_frame.is_mouse_pressed(1),
        });

        let Some(navigation) = self.navigation.as_mut() else { return; };
        let output = navigation.handle_input(UiNodeNavigationInput {
            nav_x: input.actions.ui_nav[0],
            nav_y: input.actions.ui_nav[1],
            accept: input.actions.ui_accept,
            back: input.actions.ui_back,
            hit_test,
        });

        self.apply_navigation_runtime_output(output, frame_index);
    }

    fn apply_navigation_runtime_output(&mut self, output: UiNodeNavigationOutput, frame_index: u64) {
        if output.selection_changed {
            audio(AudioFeedbackKind::UiNavigate, frame_index);
        }

        for feedback in &output.feedback {
            self.flash_navigation_feedback(feedback);
        }

        for dispatch in output.route_dispatches {
            self.dispatch_navigation_route(dispatch, frame_index);
        }

        if output.close_requested {
            self.open = false;
            self.awaiting_rebind = None;
            if self.feedback.is_none() {
                self.flash_feedback("Resume", "Returning to gameplay", UiNodeMessageSeverity::Success);
            }
        }
    }
}
