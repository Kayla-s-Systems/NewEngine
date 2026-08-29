#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_audio_api::AudioFeedbackKind;
use newengine_input_bindings_api::{
    InputBinding, InputBindingDevice, InputBindingPhase, InputBindingRegistration,
};
use newengine_ui_api::UiInputFrame;

use super::super::input::ViewportInputSnap;
use super::*;

impl RenderUiNodeSurfaceState {
    pub(super) fn process_rebind_capture(
        &mut self,
        surface_input: Option<&UiInputFrame>,
        input: &ViewportInputSnap,
        frame_index: u64,
    ) {
        if input.actions.ui_back || input.actions.ui_toggle {
            self.awaiting_rebind = None;
            audio(AudioFeedbackKind::UiBack, frame_index);
            return;
        }
        let Some(pending) = self.awaiting_rebind.clone() else {
            return;
        };
        let Some(input_frame) = surface_input else {
            return;
        };

        if let Some(&code) = input_frame.keys_pressed.iter().next() {
            if code == newengine_input_api::key_code::ESCAPE {
                self.awaiting_rebind = None;
                audio(AudioFeedbackKind::UiBack, frame_index);
                return;
            }
            let registration = InputBindingRegistration {
                binding: InputBinding::keyboard_pressed(pending.action_id.as_str(), code),
                replace_existing_for_action_device: true,
            };
            self.apply_rebind_registration(registration, &pending, "keyboard", frame_index);
            return;
        }

        if let Some(button) = input_frame.gamepad_buttons_pressed.iter().next() {
            let registration = InputBindingRegistration {
                binding: InputBinding::gamepad_button_pressed(
                    pending.action_id.as_str(),
                    button.clone(),
                ),
                replace_existing_for_action_device: true,
            };
            self.apply_rebind_registration(registration, &pending, "gamepad", frame_index);
            return;
        }

        if let Some(&button) = input_frame.mouse_pressed.iter().next() {
            let registration = InputBindingRegistration {
                binding: InputBinding {
                    action: pending.action_id.clone(),
                    device: InputBindingDevice::MouseButton,
                    code: button,
                    name: None,
                    phase: InputBindingPhase::Pressed,
                },
                replace_existing_for_action_device: true,
            };
            self.apply_rebind_registration(registration, &pending, "mouse", frame_index);
        }
    }

    fn apply_rebind_registration(
        &mut self,
        registration: InputBindingRegistration,
        pending: &PendingRebind,
        device_label: &str,
        frame_index: u64,
    ) {
        match newengine_input_bindings_runtime::register_input_binding(registration) {
            Ok(profile) => {
                self.profile = profile;
                self.flash_feedback(
                    "Binding updated",
                    format!(
                        "{} now uses the selected {} input",
                        pending.label, device_label
                    ),
                    UiNodeMessageSeverity::Success,
                );
                audio(AudioFeedbackKind::UiConfirm, frame_index);
            }
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "UI surface ui node command router: rebind rejected action='{}' err='{}'",
                    pending.action_id,
                    e
                );
                self.flash_feedback("Rebind failed", e, UiNodeMessageSeverity::Danger);
                audio(AudioFeedbackKind::UiError, frame_index);
            }
        }
        self.awaiting_rebind = None;
    }
}
