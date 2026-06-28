use newengine_core::{EngineStartupPhase, EngineStartupSnapshot};
use newengine_system_contracts::{
    ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlaySubsystem,
};
use newengine_system_runtime::overlay_from_engine_startup_snapshot;

pub(crate) struct FatalOverlayInput<'a> {
    pub startup: &'a EngineStartupSnapshot,
    pub message: &'a str,
    pub platform_window_ready: bool,
    pub render_backend_label: &'a str,
    pub loaded_engine_plugins: Option<usize>,
    pub subsystems: Vec<ScreenOverlaySubsystem>,
}

pub(crate) fn build_fatal_bootstrap_overlay(input: FatalOverlayInput<'_>) -> ScreenOverlayStatus {
    if input.startup.error.is_some() || input.startup.phase == EngineStartupPhase::Faulted {
        let mut overlay = overlay_from_engine_startup_snapshot(
            input.startup,
            input.platform_window_ready,
            input.render_backend_label,
            input.loaded_engine_plugins,
        );
        if fatal_error_looks_renderer_owned(input.message) {
            overlay.reason = ScreenOverlayReason::RenderBackendInit;
            overlay.status = "Renderer bootstrap failed.".to_owned();
            overlay.detail = input.message.to_owned();
        }
        return overlay;
    }

    let reason = if fatal_error_looks_renderer_owned(input.message) {
        ScreenOverlayReason::RenderBackendInit
    } else {
        ScreenOverlayReason::Recovery
    };
    let status = if reason == ScreenOverlayReason::RenderBackendInit {
        "Renderer bootstrap failed."
    } else {
        "Startup failed before playable handoff."
    };

    ScreenOverlayStatus::error(reason, status, input.message).with_subsystems(input.subsystems)
}

fn fatal_error_looks_renderer_owned(message: &str) -> bool {
    let text = message.to_ascii_lowercase();
    text.contains("renderer")
        || text.contains("vulkan")
        || text.contains("engine.render")
        || text.contains("shader")
        || text.contains("swapchain")
        || text.contains("surface")
        || text.contains("gpu")
}
