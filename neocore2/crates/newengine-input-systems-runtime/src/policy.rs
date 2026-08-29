#![forbid(unsafe_op_in_unsafe_fn)]

use super::InputRuntimeSystem;

/// Per-frame capture state published by UI/gameplay policy.
///
/// Contract:
/// - listeners/samplers stay alive and keep receiving raw input every frame;
/// - navigation/gameplay application is gated by policy;
/// - UI must publish capture state instead of disabling camera sampling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputCaptureState {
    pub sampling_alive: bool,
    pub camera_navigation_gated: bool,
    pub gameplay_movement_gated: bool,
    pub reason: &'static str,
}

impl Default for InputCaptureState {
    #[inline]
    fn default() -> Self {
        Self::clear()
    }
}

impl InputCaptureState {
    #[inline]
    pub const fn clear() -> Self {
        Self {
            sampling_alive: true,
            camera_navigation_gated: false,
            gameplay_movement_gated: false,
            reason: "clear",
        }
    }

    #[inline]
    pub const fn modal_ui(blocks_gameplay: bool) -> Self {
        if blocks_gameplay {
            Self {
                sampling_alive: true,
                camera_navigation_gated: true,
                gameplay_movement_gated: true,
                reason: "engine.ui.modal",
            }
        } else {
            Self::clear()
        }
    }

    #[inline]
    pub const fn has_runtime_gate(self) -> bool {
        self.camera_navigation_gated || self.gameplay_movement_gated
    }

    #[inline]
    pub const fn gates(self, system: InputRuntimeSystem) -> bool {
        match system {
            InputRuntimeSystem::CameraLook => self.camera_navigation_gated,
            InputRuntimeSystem::GameplayMovement => self.gameplay_movement_gated,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct RuntimeInputCapturePolicy {
    state: InputCaptureState,
}

impl RuntimeInputCapturePolicy {
    #[inline]
    pub(super) fn captures(&self, system: InputRuntimeSystem) -> bool {
        self.state.gates(system)
    }

    #[inline]
    pub(super) fn set_capture_state(&mut self, state: InputCaptureState) -> bool {
        let changed = self.state != state;
        self.state = state;
        changed
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn set_modal_ui_capture(&mut self, blocks_gameplay: bool) -> bool {
        self.set_capture_state(InputCaptureState::modal_ui(blocks_gameplay))
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn state(&self) -> InputCaptureState {
        self.state
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SystemObservation {
    pub(super) active: bool,
    pub(super) captured: bool,
    pub(super) reason: &'static str,
}

impl SystemObservation {
    #[inline]
    pub(super) const fn new(active: bool, captured: bool, reason: &'static str) -> Self {
        Self {
            active,
            captured,
            reason,
        }
    }
}
