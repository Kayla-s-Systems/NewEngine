#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_system_contracts::{
    ScreenOverlayProgress, ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlayStatusKind,
};

pub fn bootstrap_loading(
    title: impl Into<String>,
    status: impl Into<String>,
    detail: impl Into<String>,
    progress_01: f32,
) -> ScreenOverlayStatus {
    ScreenOverlayStatus::new(
        ScreenOverlayStatusKind::Loading,
        ScreenOverlayReason::PluginDiscovery,
        title,
        status,
        detail,
        Some(ScreenOverlayProgress::percent(progress_01)),
        false,
    )
}

pub fn runtime_ready(status: impl Into<String>, detail: impl Into<String>) -> ScreenOverlayStatus {
    ScreenOverlayStatus::new(
        ScreenOverlayStatusKind::Ready,
        ScreenOverlayReason::PlatformWindow,
        "NEWENGINE // READY",
        status,
        detail,
        Some(ScreenOverlayProgress::percent(1.0)),
        true,
    )
}

pub fn applying(
    status: impl Into<String>,
    detail: impl Into<String>,
    progress_01: f32,
) -> ScreenOverlayStatus {
    ScreenOverlayStatus::new(
        ScreenOverlayStatusKind::Applying,
        ScreenOverlayReason::StagedFilesystemApply,
        "NEWENGINE // APPLYING",
        status,
        detail,
        Some(ScreenOverlayProgress::percent(progress_01)),
        false,
    )
}

pub fn syncing(
    status: impl Into<String>,
    detail: impl Into<String>,
    progress_01: f32,
) -> ScreenOverlayStatus {
    ScreenOverlayStatus::new(
        ScreenOverlayStatusKind::Syncing,
        ScreenOverlayReason::StreamingInstall,
        "NEWENGINE // SYNCHRONIZING",
        status,
        detail,
        Some(ScreenOverlayProgress::percent(progress_01)),
        false,
    )
}
