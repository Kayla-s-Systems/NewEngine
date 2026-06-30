#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_core::loading::{
    BootFrameDto, BootViewport, LoadingPhase, ResolvedLoadingAssignment,
};
use newengine_platform_api::{
    PlatformLoadingOverlayV1, PlatformStepResultV1, PlatformSurfaceMetricsV1,
};
use newengine_system_contracts::ScreenOverlayStatus;

pub(crate) fn overlay_to_boot_step_result(
    overlay: &ScreenOverlayStatus,
    spinner_phase: u32,
    surface: PlatformSurfaceMetricsV1,
) -> PlatformStepResultV1 {
    let frame = overlay_to_boot_frame(overlay, spinner_phase, surface);
    PlatformStepResultV1 {
        exit_requested: false,
        loading_overlay: boot_frame_to_platform_overlay(&frame),
    }
}

pub(crate) fn overlay_to_boot_frame(
    overlay: &ScreenOverlayStatus,
    spinner_phase: u32,
    surface: PlatformSurfaceMetricsV1,
) -> BootFrameDto {
    let viewport = BootViewport {
        width: surface.width.max(1) as f32,
        height: surface.height.max(1) as f32,
        scale: surface.pixels_per_point.max(0.01),
    };

    BootFrameDto::from_status(
        ResolvedLoadingAssignment::engine_default(LoadingPhase::RuntimeLoading),
        viewport,
        overlay.title.as_str(),
        overlay.status.as_str(),
        overlay.detail.as_str(),
        overlay.progress_01(),
        spinner_phase,
    )
}

pub(crate) fn boot_frame_to_platform_overlay(frame: &BootFrameDto) -> PlatformLoadingOverlayV1 {
    let view_json = serde_json::to_string(frame).unwrap_or_else(|err| {
        newengine_ulog_api::ulog::warn!(
            "platform boot presenter: boot frame serialization failed err='{}'",
            err
        );
        String::new()
    });

    PlatformLoadingOverlayV1 {
        active: true,
        progress_01: frame.progress.progress_01,
        spinner_phase: frame.progress.spinner_phase,
        title: RString::from("NORTH STAR ENGINE // BOOTSTRAP"),
        status: RString::from(frame.progress.status.as_str()),
        detail: RString::from(frame.progress.detail.as_str()),
        view_json: RString::from(view_json.as_str()),
    }
}
