#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_core::loading::{
    BootFrameDto, BootViewport, LoadingPhase, LoadingProfile, ResolvedLoadingAssignment,
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
        loading_overlay: boot_frame_to_platform_overlay(&frame, overlay),
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

    let loading_profile = LoadingProfile::from_last_startup_config_or_default();
    let assignment =
        ResolvedLoadingAssignment::from_profile(LoadingPhase::RuntimeLoading, &loading_profile);

    BootFrameDto::from_status(
        assignment,
        viewport,
        overlay.title.as_str(),
        overlay.status.as_str(),
        overlay.detail.as_str(),
        overlay.progress_01(),
        spinner_phase,
    )
}

pub(crate) fn boot_frame_to_platform_overlay(
    frame: &BootFrameDto,
    overlay: &ScreenOverlayStatus,
) -> PlatformLoadingOverlayV1 {
    // Preserve both the renderer-facing boot frame and the semantic overlay state.
    // The native winit presenter parses `state.subsystems` from this envelope so
    // detailed startup-system tracing remains available before engine.ui is online.
    let payload = serde_json::json!({
        "schema": "newengine.loading.boot-presentation.v2",
        "state": overlay,
        "frame": frame,
    });
    let view_json = serde_json::to_string(&payload).unwrap_or_else(|err| {
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
        title: RString::from(overlay.title.as_str()),
        status: RString::from(frame.progress.status.as_str()),
        detail: RString::from(frame.progress.detail.as_str()),
        view_json: RString::from(view_json.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_system_contracts::{
        ScreenOverlayProgress, ScreenOverlayReason, ScreenOverlayStatusKind,
        ScreenOverlaySubsystem, ScreenOverlaySubsystemId, ScreenOverlaySubsystemPhase,
    };

    #[test]
    fn boot_overlay_envelope_preserves_semantic_subsystems() {
        let overlay = ScreenOverlayStatus::new(
            ScreenOverlayStatusKind::Loading,
            ScreenOverlayReason::JobSystem,
            "NORTH STAR ENGINE // BOOTSTRAP",
            "Initializing modules...",
            "Runtime startup graph is advancing.",
            Some(ScreenOverlayProgress::percent(0.42)),
            false,
        )
        .with_subsystems(vec![ScreenOverlaySubsystem::new(
            ScreenOverlaySubsystemId::Simulation,
            "MODULES",
            ScreenOverlaySubsystemPhase::Running,
            "INIT",
            "Processing renderer.bootstrap.",
            Some(ScreenOverlayProgress::percent(0.5)),
        )]);

        let result = overlay_to_boot_step_result(&overlay, 7, PlatformSurfaceMetricsV1::default());
        let value: serde_json::Value =
            serde_json::from_str(result.loading_overlay.view_json.as_str()).unwrap();

        assert_eq!(
            value.get("schema").and_then(serde_json::Value::as_str),
            Some("newengine.loading.boot-presentation.v2")
        );
        assert_eq!(
            value["state"]["subsystems"][0]["label"].as_str(),
            Some("MODULES")
        );
        assert_eq!(
            value["state"]["subsystems"][0]["state_label"].as_str(),
            Some("INIT")
        );
        assert!(value.get("frame").is_some());
    }

    #[test]
    fn platform_overlay_preserves_the_semantic_title() {
        let overlay = ScreenOverlayStatus::new(
            ScreenOverlayStatusKind::Loading,
            ScreenOverlayReason::JobSystem,
            "GAME FPS // LOADING",
            "Preparing...",
            "Loading authored content.",
            Some(ScreenOverlayProgress::percent(0.2)),
            false,
        );

        let result = overlay_to_boot_step_result(&overlay, 1, PlatformSurfaceMetricsV1::default());
        assert_eq!(result.loading_overlay.title.as_str(), "GAME FPS // LOADING");
    }
}
