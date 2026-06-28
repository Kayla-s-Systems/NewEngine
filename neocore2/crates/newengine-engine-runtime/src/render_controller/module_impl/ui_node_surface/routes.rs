#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_audio_api::AudioFeedbackKind;
use newengine_input_bindings_api::InputDevicePreference;
use newengine_ui_navigation_api::UiNodeActionRoute;
use newengine_ui_navigation_api::UiNodeRouteDispatch;

use super::*;

impl RenderUiNodeSurfaceState {
    pub(super) fn dispatch_navigation_route(
        &mut self,
        dispatch: UiNodeRouteDispatch,
        frame_index: u64,
    ) {
        if let Some(audio_id) = dispatch.route.audio.as_deref() {
            audio(audio_feedback_from_route(audio_id), frame_index);
        }

        let UiNodeRouteDispatch {
            route,
            source_label,
            ..
        } = dispatch;
        match (route.target.as_str(), route.event.as_str()) {
            (TARGET_SYSTEM_COMMAND, EVENT_ENGINE_SHUTDOWN_REQUEST) => {
                self.exit_requested = true;
                self.open = false;
            }
            (TARGET_INPUT_BINDINGS, EVENT_INPUT_DEVICE_NEXT) => {
                self.cycle_device_preference(1);
            }
            (TARGET_INPUT_BINDINGS, EVENT_INPUT_DEVICE_PREVIOUS) => {
                self.cycle_device_preference(-1);
            }
            (TARGET_INPUT_BINDINGS, EVENT_INPUT_BINDINGS_RESET) => {
                self.reset_bindings_profile();
            }
            (TARGET_INPUT_BINDINGS, EVENT_INPUT_BINDING_REBIND_BEGIN) => {
                self.begin_binding_rebind(&route, source_label);
            }
            (_, _) => {
                if route.target != "UiNodeNavigationRuntime" {
                    newengine_ulog_api::ulog::warn!(
                        "UI surface ui node command router: unsupported route target='{}' event='{}' id='{}'",
                        route.target,
                        route.event,
                        route.id
                    );
                    self.flash_feedback(
                        "Unavailable",
                        "This UI node action has no command route",
                        UiNodeMessageSeverity::Danger,
                    );
                    audio(AudioFeedbackKind::UiError, frame_index);
                }
            }
        }
    }

    fn begin_binding_rebind(&mut self, route: &UiNodeActionRoute, source_label: Option<String>) {
        let Some(action_id) = route.payload_str("action_id") else {
            self.flash_feedback(
                "Unavailable",
                "Binding route has no action_id payload",
                UiNodeMessageSeverity::Danger,
            );
            return;
        };
        let label = source_label.unwrap_or_else(|| action_id.to_owned());
        self.awaiting_rebind = Some(PendingRebind {
            action_id: action_id.to_owned(),
            label: label.clone(),
        });
        self.flash_feedback(
            "Listening",
            format!("Press a key, mouse button or gamepad button for {}", label),
            UiNodeMessageSeverity::Warning,
        );
    }

    fn reset_bindings_profile(&mut self) {
        match newengine_input_bindings_runtime::reset_input_bindings_profile() {
            Ok(profile) => {
                self.profile = profile;
                self.flash_feedback(
                    "Bindings reset",
                    "Default keyboard, mouse and gamepad layout restored",
                    UiNodeMessageSeverity::Success,
                );
            }
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "UI surface ui node command router: reset bindings rejected err='{}'",
                    e
                );
                self.flash_feedback("Reset failed", e, UiNodeMessageSeverity::Danger);
            }
        }
    }

    fn cycle_device_preference(&mut self, delta: i32) {
        self.profile.device_preference = match (self.profile.device_preference, delta.signum()) {
            (InputDevicePreference::KeyboardMouse, s) if s < 0 => InputDevicePreference::Hybrid,
            (InputDevicePreference::KeyboardMouse, _) => InputDevicePreference::Gamepad,
            (InputDevicePreference::Gamepad, s) if s < 0 => InputDevicePreference::KeyboardMouse,
            (InputDevicePreference::Gamepad, _) => InputDevicePreference::Hybrid,
            (InputDevicePreference::Hybrid, s) if s < 0 => InputDevicePreference::Gamepad,
            (InputDevicePreference::Hybrid, _) => InputDevicePreference::KeyboardMouse,
        };
        match newengine_input_bindings_runtime::save_input_bindings_profile(self.profile.clone()) {
            Ok(profile) => {
                self.profile = profile;
                self.flash_feedback(
                    "Input device",
                    format!(
                        "Preference: {}",
                        device_preference_label(self.profile.device_preference)
                    ),
                    UiNodeMessageSeverity::Success,
                );
            }
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "UI surface ui node command router: device preference save failed err='{}'",
                    e
                );
                self.flash_feedback("Save failed", e, UiNodeMessageSeverity::Danger);
            }
        }
    }
}
