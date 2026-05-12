#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_system_contracts::{
    ScreenOverlayProgress, ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlayStatusKind,
    ScreenOverlaySubsystem,
};

pub fn bootstrap_loading(
    title: impl Into<String>,
    status: impl Into<String>,
    detail: impl Into<String>,
    progress_01: f32,
) -> ScreenOverlayStatus {
    bootstrap_loading_with_subsystems(title, status, detail, progress_01, Vec::new())
}

pub fn bootstrap_loading_with_subsystems(
    title: impl Into<String>,
    status: impl Into<String>,
    detail: impl Into<String>,
    progress_01: f32,
    subsystems: Vec<ScreenOverlaySubsystem>,
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
    .with_subsystems(subsystems)
}

pub fn runtime_ready(status: impl Into<String>, detail: impl Into<String>) -> ScreenOverlayStatus {
    runtime_ready_with_subsystems(status, detail, Vec::new())
}

pub fn runtime_ready_with_subsystems(
    status: impl Into<String>,
    detail: impl Into<String>,
    subsystems: Vec<ScreenOverlaySubsystem>,
) -> ScreenOverlayStatus {
    ScreenOverlayStatus::new(
        ScreenOverlayStatusKind::Ready,
        ScreenOverlayReason::PlatformWindow,
        "NEWENGINE // READY",
        status,
        detail,
        Some(ScreenOverlayProgress::percent(1.0)),
        true,
    )
    .with_subsystems(subsystems)
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
