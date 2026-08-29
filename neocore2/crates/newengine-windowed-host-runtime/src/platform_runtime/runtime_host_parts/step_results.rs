use newengine_core::render::{RenderBackendStatus, SceneLaunchStatus};
use newengine_platform_api::PlatformStepResultV1;
use newengine_system_contracts::{ScreenOverlayReason, ScreenOverlayStatus};
use newengine_system_runtime::{
    overlay_from_engine_startup_snapshot, overlay_from_render_backend_status,
    overlay_to_step_result_with_provider, startup_status_mapper::bootstrap_loading_with_subsystems,
};

use super::super::HostPlatformRuntime;
use crate::platform_runtime::bootstrap_overlay::{
    map_engine_startup_progress_to_bootstrap, RuntimeBootstrapStage,
};
use crate::platform_runtime::bootstrap_subsystems::{
    build_bootstrap_subsystems, build_scene_launch_subsystems, BootstrapSubsystemInput,
    SceneLaunchSubsystemInput,
};
use crate::platform_runtime::fatal_overlay::{build_fatal_bootstrap_overlay, FatalOverlayInput};

impl HostPlatformRuntime {
    pub(crate) fn runtime_soft_degraded_step_result(&mut self) -> PlatformStepResultV1 {
        self.runtime_soft_degraded_frames = self.runtime_soft_degraded_frames.wrapping_add(1);
        let origin = self.runtime_soft_degraded_origin.unwrap_or("runtime");
        let message = self
            .runtime_soft_degraded_error
            .as_deref()
            .unwrap_or("Runtime entered recovery mode without a diagnostic message.");
        if self.runtime_soft_degraded_frames == 1 || self.runtime_soft_degraded_frames % 120 == 1 {
            newengine_ulog_api::ulog::error!(
                "platform runtime: recovery overlay active origin='{}' frames={} message='{}'",
                origin,
                self.runtime_soft_degraded_frames,
                message
            );
        }
        let overlay = ScreenOverlayStatus::error(
        ScreenOverlayReason::Recovery,
        "Runtime recovered from a frame failure.",
        format!(
            "Origin: {origin}\n{message}\nThe process is still alive; renderer is holding a safe degraded frame instead of aborting."
        ),
    )
    .with_subsystems(build_bootstrap_subsystems(BootstrapSubsystemInput {
        fatal_error: self.fatal_bootstrap_error.as_deref(),
        render_backend: self.render_backend_label(),
        loaded_engine_plugins: self.loaded_engine_plugins,
        bootstrap_stage: self.bootstrap_stage,
        bootstrap_progress: self.bootstrap_overlay.progress_01,
    }));
        self.loading_overlay_step_result(&overlay, self.runtime_soft_degraded_frames as u32)
    }

    pub(crate) fn scene_launch_step_result(
        &mut self,
        status: &SceneLaunchStatus,
    ) -> PlatformStepResultV1 {
        self.bootstrap_spinner_phase = self.bootstrap_spinner_phase.wrapping_add(1);
        let overlay = self.scene_launch_overlay(status);
        let step = crate::platform_runtime::boot_presenter::overlay_to_boot_step_result(
            &overlay,
            self.bootstrap_spinner_phase,
            self.surface,
        );
        if self.bootstrap_spinner_phase % 120 == 1 {
            newengine_ulog_api::ulog::debug!(
                "platform loading overlay: presenter=native-platform retained_ui_loading=disabled"
            );
        }
        step
    }

    pub(crate) fn scene_launch_overlay(&self, status: &SceneLaunchStatus) -> ScreenOverlayStatus {
        bootstrap_loading_with_subsystems(
            status.title.as_str(),
            status.status.as_str(),
            status.detail.as_str(),
            status.progress_01,
            build_scene_launch_subsystems(SceneLaunchSubsystemInput {
                status,
                render_backend: self.render_backend_label(),
            }),
        )
    }

    pub(crate) fn degraded_backend_step_result(
        &self,
        status: &RenderBackendStatus,
    ) -> PlatformStepResultV1 {
        match overlay_from_render_backend_status(status) {
            Some(overlay) => self.overlay_step_result(&overlay, 0),
            None => PlatformStepResultV1::default(),
        }
    }

    pub(crate) fn overlay_step_result(
        &self,
        overlay: &ScreenOverlayStatus,
        spinner_phase: u32,
    ) -> PlatformStepResultV1 {
        overlay_to_step_result_with_provider(
            overlay,
            spinner_phase,
            self.overlay_provider_binding(),
        )
    }

    pub(crate) fn loading_overlay_step_result(
        &self,
        overlay: &ScreenOverlayStatus,
        spinner_phase: u32,
    ) -> PlatformStepResultV1 {
        if !self.runtime_bootstrap_overlay_enabled {
            if spinner_phase % 120 == 1 {
                newengine_ulog_api::ulog::debug!(
                    "bootstrap loading overlay: disabled by runtime host boot option; startup continues without visual bootstrap surface"
                );
            }
            return PlatformStepResultV1::default();
        }

        // Loading presentation has exactly one owner: the platform-native loader.
        // The host emits semantic progress/subsystem telemetry only; engine.ui never
        // mounts an alternative fullscreen loading surface.
        crate::platform_runtime::boot_presenter::overlay_to_boot_step_result(
            overlay,
            spinner_phase,
            self.surface,
        )
    }

    pub(crate) fn loading_step_result(&self) -> PlatformStepResultV1 {
        let mut startup = self.engine.startup_status();
        if matches!(self.bootstrap_stage, RuntimeBootstrapStage::StartEngine) && startup.active {
            startup.progress_01 = map_engine_startup_progress_to_bootstrap(startup.progress_01);
            let overlay = overlay_from_engine_startup_snapshot(
                &startup,
                self.platform_window_ready(),
                self.render_backend_label(),
                self.loaded_engine_plugins,
            );
            return self.loading_overlay_step_result(&overlay, self.bootstrap_spinner_phase);
        }

        let status = self.bootstrap_overlay.status.as_str();
        let detail = self.bootstrap_overlay.detail.as_str();
        let subsystems = build_bootstrap_subsystems(BootstrapSubsystemInput {
            fatal_error: self.fatal_bootstrap_error.as_deref(),
            render_backend: self.render_backend_label(),
            loaded_engine_plugins: self.loaded_engine_plugins,
            bootstrap_stage: self.bootstrap_stage,
            bootstrap_progress: self.bootstrap_overlay.progress_01,
        });

        let overlay = bootstrap_loading_with_subsystems(
            self.bootstrap_overlay.title.as_str(),
            status,
            detail,
            self.bootstrap_overlay.progress_01,
            subsystems,
        );

        self.loading_overlay_step_result(&overlay, self.bootstrap_spinner_phase)
    }

    pub(crate) fn fatal_bootstrap_step_result(&mut self) -> PlatformStepResultV1 {
        self.bootstrap_spinner_phase = self.bootstrap_spinner_phase.wrapping_add(1);
        let message = self
            .fatal_bootstrap_error
            .as_deref()
            .unwrap_or("Startup failed before a diagnostic message was published.")
            .to_owned();
        let startup = self.engine.startup_status();
        let render_backend_label = self.render_backend_label();
        let overlay = build_fatal_bootstrap_overlay(FatalOverlayInput {
            startup: &startup,
            message: message.as_str(),
            platform_window_ready: self.platform_window_ready(),
            render_backend_label: render_backend_label.as_str(),
            loaded_engine_plugins: self.loaded_engine_plugins,
            subsystems: build_bootstrap_subsystems(BootstrapSubsystemInput {
                fatal_error: self.fatal_bootstrap_error.as_deref(),
                render_backend: self.render_backend_label(),
                loaded_engine_plugins: self.loaded_engine_plugins,
                bootstrap_stage: self.bootstrap_stage,
                bootstrap_progress: self.bootstrap_overlay.progress_01,
            }),
        });

        self.loading_overlay_step_result(&overlay, self.bootstrap_spinner_phase)
    }
    pub(crate) fn enter_runtime_soft_degraded_step(
        &mut self,
        origin: &'static str,
        message: impl Into<String>,
    ) -> PlatformStepResultV1 {
        let message = message.into();
        newengine_core::crash::record_breadcrumb(format!(
            "platform runtime: soft degradation origin='{origin}' message='{message}'"
        ));
        newengine_ulog_api::ulog::error!(
            "platform runtime: soft degradation activated origin='{}' message='{}'",
            origin,
            message
        );
        self.runtime_soft_degraded_origin = Some(origin);
        self.runtime_soft_degraded_error = Some(message.clone());
        self.engine
            .resources_mut()
            .insert(RenderBackendStatus::degraded(origin, message));
        self.runtime_soft_degraded_step_result()
    }
}
