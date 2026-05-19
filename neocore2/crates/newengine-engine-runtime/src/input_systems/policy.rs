#![forbid(unsafe_op_in_unsafe_fn)]

use super::InputRuntimeSystem;

#[derive(Clone, Debug, Default)]
pub(super) struct RuntimeInputCapturePolicy {
    pause_menu_blocks_gameplay: bool,
}

impl RuntimeInputCapturePolicy {
    #[inline]
    pub(super) fn captures(&self, system: InputRuntimeSystem) -> bool {
        self.pause_menu_blocks_gameplay && system.captures_runtime_controls()
    }

    #[inline]
    pub(super) fn set_pause_menu_capture(&mut self, blocks_gameplay: bool) -> bool {
        let changed = self.pause_menu_blocks_gameplay != blocks_gameplay;
        self.pause_menu_blocks_gameplay = blocks_gameplay;
        changed
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
        Self { active, captured, reason }
    }
}
