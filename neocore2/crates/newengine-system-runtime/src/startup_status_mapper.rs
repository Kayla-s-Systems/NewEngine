#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_system_contracts::{
    ScreenOverlayProgress, ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlayStatusKind,
    ScreenOverlaySubsystem,
};

/// Canonical bootstrap overlay builder.
///
/// The runtime host and scene-launch gate both publish loading overlays through
/// this single helper so the native platform shell renders one declarative model
/// instead of drifting across multiple ad-hoc status constructors.
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
