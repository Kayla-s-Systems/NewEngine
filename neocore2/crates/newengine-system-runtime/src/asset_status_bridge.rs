#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_system_contracts::{
    ScreenOverlayProgress, ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlayStatusKind,
};

pub fn asset_wait_overlay(
    label: impl Into<String>,
    detail: impl Into<String>,
    progress_01: Option<f32>,
) -> ScreenOverlayStatus {
    ScreenOverlayStatus::new(
        ScreenOverlayStatusKind::WaitingForAssets,
        ScreenOverlayReason::AssetImport,
        "NEWENGINE // ASSETS",
        label,
        detail,
        progress_01.map(ScreenOverlayProgress::percent),
        false,
    )
}
