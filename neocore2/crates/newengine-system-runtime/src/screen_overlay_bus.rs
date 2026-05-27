#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_platform_api::{PlatformLoadingOverlayV1, PlatformStepResultV1};
use newengine_system_contracts::{ScreenOverlayStatus, ScreenOverlayStatusKind};
use newengine_ui::{UiProviderBinding, UiShellSpec, UiSurfaceProjection};
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
    overlay_to_platform_overlay_with_provider(status, spinner_phase, UiProviderBinding::None)
}

pub fn overlay_to_platform_overlay_with_provider(
    status: &ScreenOverlayStatus,
    spinner_phase: u32,
    provider: UiProviderBinding,
) -> PlatformLoadingOverlayV1 {
    let view_json = serde_json::to_string(&loading_surface_projection(status, provider))
        .or_else(|projection_err| {
            log::warn!("loading UI surface projection serialization failed: {projection_err}; serializing raw screen overlay status for diagnostics only");
            serde_json::to_string(status)
        })
        .unwrap_or_else(|e| {
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
    overlay_to_step_result_with_provider(status, spinner_phase, UiProviderBinding::None)
}

pub fn overlay_to_step_result_with_provider(
    status: &ScreenOverlayStatus,
    spinner_phase: u32,
    provider: UiProviderBinding,
) -> PlatformStepResultV1 {
    PlatformStepResultV1 {
        exit_requested: false,
        loading_overlay: overlay_to_platform_overlay_with_provider(status, spinner_phase, provider),
    }
}

pub fn loading_surface_projection(
    status: &ScreenOverlayStatus,
    provider: UiProviderBinding,
) -> UiSurfaceProjection<ScreenOverlayStatus> {
    let shell = UiShellSpec::ksystems_loading();
    if status.kind == ScreenOverlayStatusKind::Error {
        UiSurfaceProjection::error_modal(provider, shell, status.clone())
    } else {
        UiSurfaceProjection::loading(provider, shell, status.clone())
    }
}
