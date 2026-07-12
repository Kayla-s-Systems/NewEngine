use super::*;

/// Provider-neutral input capture state published by UI/input policy.
///
/// Hard contract: UI can consume pointer/text/navigation, but camera sampling
/// remains alive. Runtime camera code receives an input-state frame every tick;
/// `gameplay_navigation_blocked` gates applying deltas/actions, not listener
/// subscription.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputCaptureStateV1 {
    pub camera_listener_alive: bool,
    pub gameplay_navigation_blocked: bool,
    pub ui_pointer_capture: bool,
    pub ui_text_capture: bool,
    pub ui_navigation_capture: bool,
    pub owner: String,
    pub reason: String,
}

impl Default for InputCaptureStateV1 {
    fn default() -> Self {
        Self {
            camera_listener_alive: true,
            gameplay_navigation_blocked: false,
            ui_pointer_capture: false,
            ui_text_capture: false,
            ui_navigation_capture: false,
            owner: "engine.input.contexts".to_owned(),
            reason: "observe-only".to_owned(),
        }
    }
}

impl InputCaptureStateV1 {
    #[inline]
    pub fn modal_ui(owner: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            camera_listener_alive: true,
            gameplay_navigation_blocked: true,
            ui_pointer_capture: true,
            ui_text_capture: true,
            ui_navigation_capture: true,
            owner: owner.into(),
            reason: reason.into(),
        }
    }
}
