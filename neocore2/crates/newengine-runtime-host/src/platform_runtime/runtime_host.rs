use std::path::Path;

use abi_stable::std_types::RString;
use libloading::Library;
use newengine_core::events::EventSub;
use newengine_core::render::{RenderBackendStatus, SceneLaunchStatus};
use newengine_core::EngineStartupPhase;
use newengine_core::host_events::{
    CursorGrabMode, CursorState, HostEvent, WindowHandles, WindowHostEvent, WindowInitSize,
};
use newengine_core::{Engine, EngineError, EngineResult};
use newengine_platform_api::{
    PlatformCursorGrabModeV1, PlatformCursorPollV1, PlatformCursorStateV1,
    PlatformHostApiV1, PlatformRuntimeRunFnV1,
    PlatformStepResultV1, PlatformSurfaceMetricsV1, PlatformWindowReadyV1,
};
use newengine_plugin_api::PluginInfo;
use newengine_system_contracts::{
    ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlaySubsystem, ScreenOverlaySubsystemId,
};
use newengine_system_runtime::{
    overlay_from_engine_startup_snapshot, overlay_from_render_backend_status, overlay_to_step_result,
    startup_status_mapper::{bootstrap_loading_with_subsystems, runtime_ready_with_subsystems},
};
use newengine_ui::{
    create_provider, UiBuildFn, UiFrameDesc, UiInputFrame, UiProvider, UiProviderKind,
    UiProviderOptions,
};

use crate::platform_input::poll_input_frame;
use crate::platform_runtime::callbacks::{
    host_on_close_requested_v1, host_on_window_focused_v1, host_on_window_ready_v1,
    host_on_window_resized_v1, host_poll_cursor_state_v1, host_step_v1,
};
use crate::platform_runtime::bootstrap_overlay::{
    map_engine_startup_progress_to_bootstrap, subsystem_failed, subsystem_ready, subsystem_run,
    subsystem_wait, RuntimeBootstrapOverlayState, RuntimeBootstrapStage,
    OVERLAY_LOG_PROGRESS_EPSILON, START_ENGINE_BOOTSTRAP_BASE_PROGRESS,
};
use crate::platform_runtime::constants::PLATFORM_RUNTIME_SYMBOL;
use crate::platform_runtime::handles::{native_to_raw_handles, raw_to_native_handles};
use crate::platform_runtime::snapshot_service::{
    register_platform_window_service_best_effort, update_platform_window_snapshot,
};
use crate::platform_runtime::types::ResolvedPlatformRuntimeConfig;

pub struct HostPlatformRuntime {
    engine: Engine<()>,
    ui: Box<dyn UiProvider>,
    ui_build: Option<Box<dyn UiBuildFn>>,
    host_events: EventSub<HostEvent>,
    surface: PlatformSurfaceMetricsV1,
    minimized: bool,
    started: bool,
    shutting_down: bool,
    window_ready_emitted: bool,
    bootstrap_stage: RuntimeBootstrapStage,
    bootstrap_overlay: RuntimeBootstrapOverlayState,
    bootstrap_spinner_phase: u32,
    ready_overlay_frames_left: u32,
    loaded_engine_plugins: Option<usize>,
    fatal_bootstrap_error: Option<String>,
}

impl HostPlatformRuntime {
    pub fn new(
        engine: Engine<()>,
        ui_kind: UiProviderKind,
        ui_build: Option<Box<dyn UiBuildFn>>,
    ) -> Self {
        let host_events = engine.events().subscribe::<HostEvent>();

        match &ui_kind {
            UiProviderKind::Null => {
                log::info!("ui provider: none");
            }
            UiProviderKind::Plugin { service_id } => {
                if newengine_plugin_host::has_service(service_id) {
                    log::info!(
                        "ui provider: requested plugin service='{}' is present; using plugin-backed UI when provider bridge is bound",
                        service_id
                    );
                } else {
                    log::warn!(
                        "ui provider: requested plugin service='{}' is missing; continuing without UI",
                        service_id
                    );
                }
            }
        }

        Self {
            engine,
            ui: create_provider(UiProviderOptions { kind: ui_kind }),
            ui_build,
            host_events,
            surface: PlatformSurfaceMetricsV1::default(),
            minimized: false,
            started: false,
            shutting_down: false,
            window_ready_emitted: false,
            bootstrap_stage: RuntimeBootstrapStage::AwaitingWindow,
            bootstrap_overlay: RuntimeBootstrapOverlayState::default(),
            bootstrap_spinner_phase: 0,
            ready_overlay_frames_left: 45,
            loaded_engine_plugins: None,
            fatal_bootstrap_error: None,
        }
    }

    pub fn run(
        mut self,
        runtime_path: &Path,
        resolved: &ResolvedPlatformRuntimeConfig,
    ) -> EngineResult<()> {
        let config = resolved.config.clone();

        let info = PluginInfo {
            id: RString::from(resolved.plugin_id.clone()),
            name: RString::from(resolved.plugin_name.clone()),
            version: RString::from(resolved.plugin_version.clone()),
        };

        newengine_plugin_host::register_external_runtime_plugin(
            runtime_path.to_path_buf(),
            info,
            resolved.descriptor.clone(),
            "running",
        )
        .map_err(EngineError::other)?;

        log::info!("platform runtime: loading '{}'", runtime_path.display());

        let lib = unsafe { Library::new(runtime_path) }
            .map_err(|e| EngineError::other(format!("platform runtime load failed: {e}")))?;

        let run: libloading::Symbol<PlatformRuntimeRunFnV1> =
            unsafe { lib.get(PLATFORM_RUNTIME_SYMBOL) }
                .map_err(|e| EngineError::other(format!(
                    "platform runtime symbol missing: {e}"
                )))?;

        log::info!(
            "platform runtime: entry resolved symbol='{}' title='{}' size={}x{}",
            "newengine_platform_runtime_run_v1",
            config.title,
            config.width,
            config.height
        );

        let host = PlatformHostApiV1 {
            user_data: (&mut self as *mut Self) as usize,
            on_window_ready_v1: host_on_window_ready_v1,
            on_window_resized_v1: host_on_window_resized_v1,
            on_window_focused_v1: host_on_window_focused_v1,
            on_close_requested_v1: host_on_close_requested_v1,
            step_v1: host_step_v1,
            poll_cursor_state_v1: host_poll_cursor_state_v1,
        };

        let plugin_host = newengine_plugin_host::default_host_api();
        let result = unsafe { run(plugin_host, host, config) }
            .into_result()
            .map_err(|e| EngineError::other(e.to_string()));

        self.shutdown_engine_once("platform runtime returned");

        newengine_plugin_host::host_context::unregister_by_owner(&resolved.plugin_id);

        match &result {
            Ok(()) => log::info!("platform runtime: exited cleanly"),
            Err(e) => log::error!("platform runtime: exited with error: {e}"),
        }

        result
    }

    fn shutdown_engine_once(&mut self, origin: &'static str) {
        if !self.started || self.shutting_down {
            return;
        }
        self.shutting_down = true;
        log::info!("platform runtime: engine.shutdown begin origin={origin}");
        newengine_core::crash::record_breadcrumb(format!(
            "platform runtime: engine.shutdown begin origin={origin}"
        ));
        match self.engine.shutdown() {
            Ok(()) => {
                log::info!("platform runtime: engine.shutdown completed origin={origin}");
            }
            Err(e) => {
                log::error!("platform runtime: engine.shutdown failed origin={origin}: {e}");
            }
        }
        newengine_core::crash::record_breadcrumb(format!(
            "platform runtime: engine.shutdown completed origin={origin}"
        ));
        self.started = false;
        self.shutting_down = false;
    }

    pub(crate) fn on_window_ready(&mut self, ready: PlatformWindowReadyV1) -> EngineResult<()> {
        log::info!(
            "platform runtime: window ready backend={:?} size={}x{} ppp={:.3}",
            ready.handles.backend,
            ready.surface.width,
            ready.surface.height,
            ready.surface.pixels_per_point
        );

        self.surface = ready.surface;
        register_platform_window_service_best_effort(ready);
        let (display, window) = native_to_raw_handles(ready.handles)?;

        self.engine.resources_mut().insert(WindowHandles { window, display });
        self.engine.resources_mut().insert(WindowInitSize {
            width: ready.surface.width,
            height: ready.surface.height,
        });

        if let Some(startup) = newengine_core::startup::last_startup_config() {
            std::env::set_var("NEWENGINE_RENDER_BACKEND", startup.render_backend.as_str());
        }

        self.window_ready_emitted = false;
        self.bootstrap_stage = RuntimeBootstrapStage::AnnounceLoadEnginePlugins;
        self.set_bootstrap_overlay(
            "Platform window ready.",
            "Preparing staged engine bootstrap and loading screen.",
            0.10,
        );
        log::info!(
            "platform runtime bootstrap: staged startup armed size={}x{}",
            ready.surface.width,
            ready.surface.height
        );

        Ok(())
    }

    pub(crate) fn on_window_resized(
        &mut self,
        metrics: PlatformSurfaceMetricsV1,
    ) -> EngineResult<()> {
        log::debug!(
            "platform runtime: resized {}x{} ppp={:.3}",
            metrics.width,
            metrics.height,
            metrics.pixels_per_point
        );

        self.surface = metrics;
        if let Some(handles) = self.engine.resources.get::<WindowHandles>() {
            update_platform_window_snapshot(PlatformWindowReadyV1 {
                handles: raw_to_native_handles(handles.window, handles.display)?,
                surface: metrics,
            });
        }

        self.engine.resources_mut().insert(WindowInitSize {
            width: metrics.width,
            height: metrics.height,
        });

        let minimized = metrics.width == 0 || metrics.height == 0;
        if minimized != self.minimized {
            self.minimized = minimized;
            self.engine
                .emit(HostEvent::Window(WindowHostEvent::MinimizedChanged(minimized)))?;
        }

        self.engine.emit(HostEvent::Window(WindowHostEvent::Resized {
            width: metrics.width,
            height: metrics.height,
        }))?;
        Ok(())
    }

    pub(crate) fn on_window_focused(&mut self, focused: bool) -> EngineResult<()> {
        log::debug!("platform runtime: focused={focused}");
        self.engine
            .emit(HostEvent::Window(WindowHostEvent::Focused(focused)))?;
        Ok(())
    }

    pub(crate) fn on_close_requested(&mut self) -> EngineResult<()> {
        log::info!("platform runtime: close requested");
        self.engine
            .emit(HostEvent::Window(WindowHostEvent::CloseRequested))?;
        self.engine.request_exit()?;

        // Tear down engine modules and Vulkan-backed services while the native
        // platform window is still alive. Deferring teardown until after the
        // winit app returns can leave swapchain/surface destruction racing the
        // OS window teardown and has produced STATUS_ACCESS_VIOLATION on close.
        self.shutdown_engine_once("close requested");
        Ok(())
    }

    pub(crate) fn step(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
        if self.fatal_bootstrap_error.is_some() {
            return Ok(self.fatal_bootstrap_step_result());
        }

        let bootstrap_active = self.bootstrap_stage != RuntimeBootstrapStage::Running || !self.window_ready_emitted;
        let result = if bootstrap_active {
            self.step_bootstrap()
        } else {
            self.step_running(dt_sec)
        };

        match result {
            Ok(step) => Ok(step),
            Err(e) if bootstrap_active => {
                let message = e.to_string();
                log::error!("platform runtime bootstrap: fatal startup error: {message}");
                self.fatal_bootstrap_error = Some(message);
                Ok(self.fatal_bootstrap_step_result())
            }
            Err(e) => Err(e),
        }
    }

    fn step_bootstrap(&mut self) -> EngineResult<PlatformStepResultV1> {
        self.bootstrap_spinner_phase = self.bootstrap_spinner_phase.wrapping_add(1);

        match self.bootstrap_stage {
            RuntimeBootstrapStage::AwaitingWindow => {
                self.set_bootstrap_overlay(
                    "Waiting for platform window...",
                    "The runtime shell is preparing the first visible frame.",
                    0.0,
                );
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::AnnounceLoadEnginePlugins => {
                self.set_bootstrap_overlay(
                    "Loading engine plugins...",
                    "Discovering runtime providers, services and renderer bridge.",
                    0.22,
                );
                self.bootstrap_stage = RuntimeBootstrapStage::LoadEnginePlugins;
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::LoadEnginePlugins => {
                match self.engine.load_engine_plugins_once() {
                    Ok(count) => {
                        log::info!(
                            "platform runtime: engine plugins init completed loaded_count={}",
                            count
                        );
                        self.loaded_engine_plugins = Some(count);
                        self.set_bootstrap_overlay(
                            format!("Engine plugins loaded ({count})."),
                            "Runtime services are registered. Preparing startup graph.",
                            0.56,
                        );
                        self.bootstrap_stage = RuntimeBootstrapStage::AnnounceStartEngine;
                        Ok(self.loading_step_result())
                    }
                    Err(e) => {
                        log::error!("platform runtime: engine plugins init failed: {}", e);
                        Err(e)
                    }
                }
            }
            RuntimeBootstrapStage::AnnounceStartEngine => {
                self.set_bootstrap_overlay(
                    "Starting engine modules...",
                    "Dispatching startup graph, readiness gates and scene bootstrap.",
                    0.74,
                );
                self.bootstrap_stage = RuntimeBootstrapStage::StartEngine;
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::StartEngine => {
                match self.engine.start_incremental_step() {
                    Ok(outcome) => {
                        let snapshot = outcome.snapshot;
                        let overlay_progress = map_engine_startup_progress_to_bootstrap(
                            snapshot.progress_01,
                        )
                        .clamp(START_ENGINE_BOOTSTRAP_BASE_PROGRESS, 0.94);
                        self.set_bootstrap_overlay(
                            snapshot.status.clone(),
                            snapshot.detail.clone(),
                            overlay_progress,
                        );

                        if outcome.finished {
                            self.started = true;
                            log::info!("platform runtime: engine.start incremental pump completed");
                            self.set_bootstrap_overlay(
                                "Engine runtime started.",
                                "Finalizing gated scene readiness and host window events.",
                                0.90,
                            );
                            self.bootstrap_stage = RuntimeBootstrapStage::AnnounceEnterRuntime;
                        }

                        Ok(self.loading_step_result())
                    }
                    Err(e) => {
                        log::error!("platform runtime: engine.start incremental pump failed: {}", e);
                        Err(e)
                    }
                }
            },
            RuntimeBootstrapStage::AnnounceEnterRuntime => {
                self.set_bootstrap_overlay(
                    "Preparing playable world...",
                    "Native loading remains active while scene resources become resident.",
                    0.95,
                );
                self.bootstrap_stage = RuntimeBootstrapStage::EmitWindowReady;
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::EmitWindowReady => {
                self.emit_window_ready_event()?;
                self.window_ready_emitted = true;
                self.set_bootstrap_overlay(
                    "Loading world resources...",
                    "Player control and world presentation are locked behind the scene launch gate.",
                    0.96,
                );
                self.ready_overlay_frames_left = 1;
                self.bootstrap_stage = RuntimeBootstrapStage::ReadyOverlay;
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::ReadyOverlay => {
                let result = self.loading_step_result();
                if self.ready_overlay_frames_left == 0 {
                    self.bootstrap_stage = RuntimeBootstrapStage::Running;
                } else {
                    self.ready_overlay_frames_left = self.ready_overlay_frames_left.saturating_sub(1);
                }
                Ok(result)
            }
            RuntimeBootstrapStage::Running => self.step_running(0.0),
        }
    }

    fn step_running(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
        let input_frame = poll_input_frame();
        if let Some(input) = input_frame.clone() {
            self.engine.resources_mut().insert::<UiInputFrame>(input);
        } else {
            let _ = self.engine.resources_mut().remove::<UiInputFrame>();
        }

        if let Some(build) = self.ui_build.as_deref_mut() {
            let mut desc = UiFrameDesc::new(dt_sec).with_surface(
                self.surface.width,
                self.surface.height,
                self.surface.pixels_per_point,
            );

            if let Some(input) = input_frame {
                desc = desc.with_input(input);
            }

            let out = self.ui.run_frame(&(), desc, build);
            self.engine.resources_mut().insert(out.draw_list);
        }

        match self.engine.step() {
            Ok(()) => {
                if let Some(status) = self.engine.resources.get::<SceneLaunchStatus>().cloned() {
                    if status.active {
                        return Ok(self.scene_launch_step_result(&status));
                    }
                }

                if let Some(status) = self.engine.resources.get::<RenderBackendStatus>() {
                    if status.degraded {
                        return Ok(self.degraded_backend_step_result(status));
                    }
                }
                Ok(PlatformStepResultV1::default())
            }
            Err(EngineError::ExitRequested) => Ok(PlatformStepResultV1 {
                exit_requested: true,
                ..PlatformStepResultV1::default()
            }),
            Err(e) => Err(e),
        }
    }


    fn scene_launch_step_result(&mut self, status: &SceneLaunchStatus) -> PlatformStepResultV1 {
        self.bootstrap_spinner_phase = self.bootstrap_spinner_phase.wrapping_add(1);
        let overlay = bootstrap_loading_with_subsystems(
            status.title.as_str(),
            status.status.as_str(),
            status.detail.as_str(),
            status.progress_01,
            self.scene_launch_subsystems(status),
        );
        overlay_to_step_result(&overlay, self.bootstrap_spinner_phase)
    }


    fn degraded_backend_step_result(&self, status: &RenderBackendStatus) -> PlatformStepResultV1 {
        match overlay_from_render_backend_status(status) {
            Some(overlay) => overlay_to_step_result(&overlay, 0),
            None => PlatformStepResultV1::default(),
        }
    }

    fn emit_window_ready_event(&mut self) -> EngineResult<()> {
        log::info!(
            "platform runtime bootstrap: emitting WindowReady width={} height={}",
            self.surface.width,
            self.surface.height
        );
        self.engine.emit(HostEvent::Window(WindowHostEvent::Ready {
            width: self.surface.width,
            height: self.surface.height,
        }))
    }

    fn set_bootstrap_overlay(
        &mut self,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
    ) {
        let next_status = status.into();
        let next_detail = detail.into();
        let requested_progress = progress_01.clamp(0.0, 1.0);
        let next_progress = requested_progress.max(self.bootstrap_overlay.progress_01);

        let text_changed = self.bootstrap_overlay.status != next_status
            || self.bootstrap_overlay.detail != next_detail;
        let progress_changed = (self.bootstrap_overlay.progress_01 - next_progress).abs()
            >= OVERLAY_LOG_PROGRESS_EPSILON;

        self.bootstrap_overlay.status = next_status;
        self.bootstrap_overlay.detail = next_detail;
        self.bootstrap_overlay.progress_01 = next_progress;

        if text_changed || progress_changed {
            log::info!(
                "platform runtime bootstrap: overlay status='{}' detail='{}' progress={:.0}%",
                self.bootstrap_overlay.status,
                self.bootstrap_overlay.detail,
                self.bootstrap_overlay.progress_01 * 100.0
            );
        }
    }

    fn loading_step_result(&self) -> PlatformStepResultV1 {
        let mut startup = self.engine.startup_status();
        if matches!(self.bootstrap_stage, RuntimeBootstrapStage::StartEngine) && startup.active {
            startup.progress_01 = map_engine_startup_progress_to_bootstrap(startup.progress_01);
            let overlay = overlay_from_engine_startup_snapshot(
                &startup,
                self.platform_window_ready(),
                self.render_backend_label(),
                self.loaded_engine_plugins,
            );
            return overlay_to_step_result(&overlay, self.bootstrap_spinner_phase);
        }

        let status = self.bootstrap_overlay.status.as_str();
        let detail = self.bootstrap_overlay.detail.as_str();
        let subsystems = self.bootstrap_subsystems();

        let overlay = if self.bootstrap_overlay.progress_01 >= 0.999 {
            runtime_ready_with_subsystems(status, detail, subsystems)
        } else {
            bootstrap_loading_with_subsystems(
                self.bootstrap_overlay.title.as_str(),
                status,
                detail,
                self.bootstrap_overlay.progress_01,
                subsystems,
            )
        };

        overlay_to_step_result(&overlay, self.bootstrap_spinner_phase)
    }

    fn fatal_bootstrap_step_result(&mut self) -> PlatformStepResultV1 {
        self.bootstrap_spinner_phase = self.bootstrap_spinner_phase.wrapping_add(1);
        let message = self
            .fatal_bootstrap_error
            .as_deref()
            .unwrap_or("Startup failed before a diagnostic message was published.")
            .to_owned();
        let startup = self.engine.startup_status();

        let overlay = if startup.error.is_some() || startup.phase == EngineStartupPhase::Faulted {
            overlay_from_engine_startup_snapshot(
                &startup,
                self.platform_window_ready(),
                self.render_backend_label(),
                self.loaded_engine_plugins,
            )
        } else {
            ScreenOverlayStatus::error(
                ScreenOverlayReason::Recovery,
                "Startup failed before playable handoff.",
                message.as_str(),
            )
            .with_subsystems(self.bootstrap_subsystems())
        };

        overlay_to_step_result(&overlay, self.bootstrap_spinner_phase)
    }

    #[inline]
    fn platform_window_ready(&self) -> bool {
        self.surface.width > 0 && self.surface.height > 0 && self.bootstrap_stage != RuntimeBootstrapStage::AwaitingWindow
    }


    fn bootstrap_subsystems(&self) -> Vec<ScreenOverlaySubsystem> {
        if let Some(error) = self.fatal_bootstrap_error.as_deref() {
            let render_backend = self.render_backend_label();
            return vec![
                subsystem_ready(ScreenOverlaySubsystemId::Platform, "READY", "Native window remained alive for safe-stop diagnostics."),
                subsystem_ready(ScreenOverlaySubsystemId::Assets, "READY", "Asset service state was already published before the failure, or is not the failing gate."),
                subsystem_run(ScreenOverlaySubsystemId::Renderer, render_backend, "Renderer state is preserved while the loading screen reports the failure.", None),
                subsystem_failed(ScreenOverlaySubsystemId::Simulation, "ERR", "Engine startup FSM did not reach playable runtime."),
                subsystem_failed(ScreenOverlaySubsystemId::Diagnostics, "ERR", error),
            ];
        }

        let render_backend = self.render_backend_label();
        let plugin_detail = self
            .loaded_engine_plugins
            .map(|count| format!("{count} engine plugin service(s) loaded."))
            .unwrap_or_else(|| "Engine plugin services are not loaded yet.".to_owned());

        match self.bootstrap_stage {
            RuntimeBootstrapStage::AwaitingWindow => vec![
                subsystem_wait(ScreenOverlaySubsystemId::Platform, "WINDOW", "Waiting for the platform window callback."),
                subsystem_wait(ScreenOverlaySubsystemId::Assets, "WAIT", "AssetManager is not guaranteed to be online yet."),
                subsystem_wait(ScreenOverlaySubsystemId::Renderer, "WAIT", "Renderer backend starts after window handles are published."),
                subsystem_wait(ScreenOverlaySubsystemId::Simulation, "WAIT", "Simulation modules are blocked by bootstrap."),
                subsystem_run(ScreenOverlaySubsystemId::Diagnostics, "BOOT", "Runtime-host bootstrap is alive.", None),
            ],
            RuntimeBootstrapStage::AnnounceLoadEnginePlugins | RuntimeBootstrapStage::LoadEnginePlugins => vec![
                subsystem_ready(ScreenOverlaySubsystemId::Platform, "READY", "Native window and surface metrics are available."),
                subsystem_run(ScreenOverlaySubsystemId::Assets, "SERVICES", "Loading AssetManager/importer services through plugin host.", Some(self.bootstrap_overlay.progress_01)),
                subsystem_wait(ScreenOverlaySubsystemId::Renderer, "WAIT", "Renderer backend is waiting for engine plugin services."),
                subsystem_wait(ScreenOverlaySubsystemId::Simulation, "WAIT", "Simulation starts after engine plugin discovery."),
                subsystem_run(ScreenOverlaySubsystemId::Diagnostics, "CHECKING", "Plugin discovery and capability checks are running.", None),
            ],
            RuntimeBootstrapStage::AnnounceStartEngine | RuntimeBootstrapStage::StartEngine => vec![
                subsystem_ready(ScreenOverlaySubsystemId::Platform, "READY", "Native window and surface metrics are available."),
                subsystem_ready(ScreenOverlaySubsystemId::Assets, "READY", plugin_detail),
                subsystem_run(ScreenOverlaySubsystemId::Renderer, render_backend, "Renderer backend is being bound to runtime resources.", None),
                subsystem_run(ScreenOverlaySubsystemId::Simulation, "STARTING", "Engine startup graph is dispatching modules.", Some(self.bootstrap_overlay.progress_01)),
                subsystem_run(ScreenOverlaySubsystemId::Diagnostics, "CHECKING", "Startup readiness gates are being evaluated.", None),
            ],
            RuntimeBootstrapStage::AnnounceEnterRuntime | RuntimeBootstrapStage::EmitWindowReady | RuntimeBootstrapStage::ReadyOverlay => vec![
                subsystem_ready(ScreenOverlaySubsystemId::Platform, "READY", "WindowReady event is emitted to the engine host."),
                subsystem_ready(ScreenOverlaySubsystemId::Assets, "READY", plugin_detail),
                subsystem_ready(ScreenOverlaySubsystemId::Renderer, render_backend, "Renderer backend is available for the first world frame."),
                subsystem_run(ScreenOverlaySubsystemId::Simulation, "HANDOFF", "Scene launch gate owns final playable-world readiness.", Some(self.bootstrap_overlay.progress_01)),
                subsystem_run(ScreenOverlaySubsystemId::Diagnostics, "CHECKING", "Final handoff diagnostics are collecting runtime status.", None),
            ],
            RuntimeBootstrapStage::Running => vec![
                subsystem_ready(ScreenOverlaySubsystemId::Platform, "READY", "Platform runtime is running."),
                subsystem_ready(ScreenOverlaySubsystemId::Assets, "READY", "Asset services are online."),
                subsystem_ready(ScreenOverlaySubsystemId::Renderer, render_backend, "Renderer backend is active."),
                subsystem_ready(ScreenOverlaySubsystemId::Simulation, "READY", "Simulation is accepting frame ticks."),
                subsystem_ready(ScreenOverlaySubsystemId::Diagnostics, "READY", "Bootstrap diagnostics are complete."),
            ],
        }
    }

    fn scene_launch_subsystems(&self, status: &SceneLaunchStatus) -> Vec<ScreenOverlaySubsystem> {
        let progress = status.progress_01.clamp(0.0, 0.995);
        let render_backend = self.render_backend_label();
        let assets_ready = progress >= 0.96 || !status.detail.to_ascii_lowercase().contains("waiting");
        let simulation_ready = progress >= 0.90;

        vec![
            subsystem_ready(ScreenOverlaySubsystemId::Platform, "READY", "Platform window remains alive while launch gate is active."),
            if assets_ready {
                subsystem_ready(ScreenOverlaySubsystemId::Assets, "READY", status.detail.clone())
            } else {
                subsystem_run(ScreenOverlaySubsystemId::Assets, "STREAMING", status.detail.clone(), Some(progress))
            },
            subsystem_ready(ScreenOverlaySubsystemId::Renderer, render_backend, "Renderer backend accepted the launch scene frame package."),
            if simulation_ready {
                subsystem_ready(ScreenOverlaySubsystemId::Simulation, "READY", "Simulation handoff is ready for playable control.")
            } else {
                subsystem_run(ScreenOverlaySubsystemId::Simulation, "LOCKED", "Player control is locked until the scene launch gate opens.", Some(progress))
            },
            subsystem_run(ScreenOverlaySubsystemId::Diagnostics, "CHECKING", status.status.clone(), Some(progress)),
        ]
    }

    fn render_backend_label(&self) -> &'static str {
        let Some(backend) = newengine_core::startup::last_startup_config()
            .map(|startup| startup.render_backend.as_str().to_ascii_lowercase())
        else {
            return "WAIT";
        };

        if backend.contains("vulkan") || backend == "vk" || backend.contains("ash") {
            "VULKAN"
        } else if backend.contains("null") {
            "NULL"
        } else if backend.contains("mock") {
            "MOCK"
        } else {
            "RENDER"
        }
    }

    pub(crate) fn poll_cursor_state(&mut self) -> PlatformCursorPollV1 {
        let mut last: Option<CursorState> = None;

        self.host_events.drain(|ev| {
            if let HostEvent::Window(WindowHostEvent::Cursor(state)) = ev.as_ref() {
                last = Some(*state);
            }
        });

        match last {
            Some(state) => PlatformCursorPollV1 {
                has_value: true,
                state: PlatformCursorStateV1 {
                    visible: state.visible,
                    grab: match state.grab {
                        CursorGrabMode::None => PlatformCursorGrabModeV1::None,
                        CursorGrabMode::Confined => PlatformCursorGrabModeV1::Confined,
                        CursorGrabMode::Locked => PlatformCursorGrabModeV1::Locked,
                    },
                },
            },
            None => PlatformCursorPollV1::default(),
        }
    }
}
