#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_platform_api::{PlatformLoadingOverlayV1, PlatformStepResultV1};
use newengine_system_contracts::ScreenOverlayStatus;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct ScreenOverlayBus {
    current: Arc<RwLock<Option<ScreenOverlayStatus>>>,
}

impl ScreenOverlayBus {
    #[inline]
    pub fn publish(&self, status: ScreenOverlayStatus) {
        *self.current.write() = Some(status);
    }

    #[inline]
    pub fn clear(&self) {
        *self.current.write() = None;
    }

    #[inline]
    pub fn snapshot(&self) -> Option<ScreenOverlayStatus> {
        self.current.read().clone()
    }
}

pub fn overlay_to_platform_overlay(
    status: &ScreenOverlayStatus,
    spinner_phase: u32,
) -> PlatformLoadingOverlayV1 {
    let view_json = serde_json::to_string(status).unwrap_or_else(|e| {
        log::warn!("screen overlay serialization failed: {e}");
        String::new()
    });

    PlatformLoadingOverlayV1 {
        active: true,
        progress_01: status.progress_01(),
        spinner_phase: if status.terminal { 0 } else { spinner_phase },
        title: RString::from(status.title.as_str()),
        status: RString::from(status.status.as_str()),
        detail: RString::from(status.detail.as_str()),
        view_json: RString::from(view_json.as_str()),
    }
}

pub fn overlay_to_step_result(
    status: &ScreenOverlayStatus,
    spinner_phase: u32,
) -> PlatformStepResultV1 {
    PlatformStepResultV1 {
        exit_requested: false,
        loading_overlay: overlay_to_platform_overlay(status, spinner_phase),
    }
}
