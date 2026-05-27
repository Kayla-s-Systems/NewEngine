use std::path::Path;

use abi_stable::std_types::RString;
use libloading::Library;
use newengine_core::events::EventSub;
use newengine_core::render::{RenderBackendStatus, SceneLaunchStatus};
use newengine_core::EngineStartupPhase;
use newengine_core::host_events::{
    CursorGrabMode, CursorState, HostEvent, WindowHandles, WindowHostEvent, WindowInitSize,
};
use newengine_core::{Engine, EngineError, EngineResult, EngineRunState, JobLane, JobPriority, JobRequest};
use newengine_platform_api::{
    PlatformCursorGrabModeV1, PlatformCursorPollV1, PlatformCursorStateV1,
    PlatformHostApiV1, PlatformHostJobCallbackV1, PlatformHostJobRequestV1, PlatformHostJobTicketV1,
    PlatformRuntimeRunFnV1, PlatformStepResultV1, PlatformSurfaceMetricsV1, PlatformWindowReadyV1,
};
use newengine_plugin_api::PluginInfo;
use newengine_system_contracts::{
    ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlaySubsystem, ScreenOverlaySubsystemId,
};
use newengine_system_runtime::{
    overlay_from_engine_startup_snapshot, overlay_from_render_backend_status,
    overlay_to_step_result_with_provider,
    startup_status_mapper::bootstrap_loading_with_subsystems,
};
use newengine_ui::{
    create_provider, UiBuildFn, UiFrameDesc, UiProvider, UiProviderKind,
    UiProviderOptions, UiProviderBinding,
};
use newengine_ui_api::{UiDrawList, UiInputFrame};

use crate::platform_input::poll_input_frame;
use crate::platform_runtime::callbacks::{
    host_on_close_requested_v1, host_on_window_focused_v1, host_on_window_ready_v1,
    host_on_window_resized_v1, host_poll_cursor_state_v1, host_step_v1, host_submit_job_v1,
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
use crate::platform_runtime::jobs_gateway::register_jobs_gateway_service_best_effort;
use crate::platform_runtime::shutdown_watchdog::ShutdownWatchdog;
use crate::platform_runtime::types::ResolvedPlatformRuntimeConfig;
use crate::render_runtime::ResolvedRenderBackendConfig;
use crate::platform_runtime::ui_provider_selection::{
    log_ui_provider_selection, UiProviderSelection,
};

pub struct HostPlatformRuntime {
    engine: Engine<()>,
    ui: Box<dyn UiProvider>,
    ui_build: Option<Box<dyn UiBuildFn>>,
    ui_selection: UiProviderSelection,
    host_events: EventSub<HostEvent>,
    surface: PlatformSurfaceMetricsV1,
    minimized: bool,
    started: bool,
    shutting_down: bool,
    close_requested: bool,
    window_ready_emitted: bool,
    last_platform_cursor: CursorState,
    force_cursor_reapply: bool,
    bootstrap_stage: RuntimeBootstrapStage,
    bootstrap_overlay: RuntimeBootstrapOverlayState,
    bootstrap_spinner_phase: u32,
    ready_overlay_frames_left: u32,
    ui_frame_index: u64,
    loaded_engine_plugins: Option<usize>,
    fatal_bootstrap_error: Option<String>,
    runtime_soft_degraded_error: Option<String>,
    runtime_soft_degraded_origin: Option<&'static str>,
    runtime_soft_degraded_frames: u64,
    cached_provider_ui_draw: Option<UiDrawList>,
}


impl HostPlatformRuntime {
    pub fn new(
        engine: Engine<()>,
        ui_kind: UiProviderKind,
        ui_build: Option<Box<dyn UiBuildFn>>,
    ) -> Self {
        let host_events = engine.events().subscribe::<HostEvent>();
        let ui_selection = UiProviderSelection::new(ui_kind);
        log_ui_provider_selection("initial", ui_selection.active());
        let active_ui_kind = ui_selection.active().clone();

        Self {
            engine,
            ui: create_provider(UiProviderOptions { kind: active_ui_kind }),
            ui_build,
            ui_selection,
            host_events,
            surface: PlatformSurfaceMetricsV1::default(),
            minimized: false,
            started: false,
            shutting_down: false,
            close_requested: false,
            window_ready_emitted: false,
            last_platform_cursor: CursorState::released(),
            force_cursor_reapply: false,
            bootstrap_stage: RuntimeBootstrapStage::AwaitingWindow,
            bootstrap_overlay: RuntimeBootstrapOverlayState::default(),
            bootstrap_spinner_phase: 0,
            ready_overlay_frames_left: 45,
            ui_frame_index: 0,
            loaded_engine_plugins: None,
            fatal_bootstrap_error: None,
            runtime_soft_degraded_error: None,
            runtime_soft_degraded_origin: None,
            runtime_soft_degraded_frames: 0,
            cached_provider_ui_draw: None,
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

        crate::platform_early_log!("host.run.enter runtime_path='{}'", runtime_path.display());
        log::info!("platform runtime: loading '{}'", runtime_path.display());

        crate::platform_early_log!("host.dll.load.begin path='{}'", runtime_path.display());
        let lib = unsafe { Library::new(runtime_path) }
            .map_err(|e| {
                crate::platform_early_log!("host.dll.load.err error='{}'", e);
                EngineError::other(format!("platform runtime load failed: {e}"))
            })?;
        crate::platform_early_log!("host.dll.load.ok path='{}'", runtime_path.display());

        crate::platform_early_log!("host.symbol.resolve.begin symbol='{}'", "newengine_platform_runtime_run_v1");
        let run: libloading::Symbol<PlatformRuntimeRunFnV1> =
            unsafe { lib.get(PLATFORM_RUNTIME_SYMBOL) }
                .map_err(|e| {
                    crate::platform_early_log!("host.symbol.resolve.err error='{}'", e);
                    EngineError::other(format!(
                        "platform runtime symbol missing: {e}"
                    ))
                })?;
        crate::platform_early_log!("host.symbol.resolve.ok symbol='{}'", "newengine_platform_runtime_run_v1");

        log::info!(
            "platform runtime: entry resolved symbol='{}' title='{}' size={}x{}",
            "newengine_platform_runtime_run_v1",
            config.title,
            config.width,
            config.height
        );

        // engine.jobs is available before the platform provider starts its
        // native bootstrap surface. Platform plugins must submit bootstrap work
        // through this callback instead of creating hidden threads.
        register_jobs_gateway_service_best_effort(self.engine.job_system(), self.engine.events().clone());

        let host = PlatformHostApiV1 {
            user_data: (&mut self as *mut Self) as usize,
            on_window_ready_v1: host_on_window_ready_v1,
            on_window_resized_v1: host_on_window_resized_v1,
            on_window_focused_v1: host_on_window_focused_v1,
            on_close_requested_v1: host_on_close_requested_v1,
            step_v1: host_step_v1,
            poll_cursor_state_v1: host_poll_cursor_state_v1,
            submit_job_v1: host_submit_job_v1,
        };

        let plugin_host = newengine_plugin_host::default_host_api();
        crate::platform_early_log!(
            "host.ffi.call.begin user_data=0x{:x} title='{}' size={}x{}",
            host.user_data,
            config.title,
            config.width,
            config.height
        );
        let result = unsafe { run(plugin_host, host, config) }
            .into_result()
            .map_err(|e| EngineError::other(e.to_string()));
        match &result {
            Ok(()) => crate::platform_early_log!("host.ffi.call.returned ok"),
            Err(e) => crate::platform_early_log!("host.ffi.call.returned err='{}'", e),
        }

        let shutdown_exit_code = if result.is_ok() { 0 } else { 1 };
        let shutdown_watchdog = ShutdownWatchdog::arm(self.engine.job_system(), "platform runtime returned", shutdown_exit_code);

        self.shutdown_engine_once("platform runtime returned");

        newengine_plugin_host::host_context::unregister_by_owner(&resolved.plugin_id);
        shutdown_watchdog.complete();

        match &result {
            Ok(()) => log::info!("platform runtime: exited cleanly"),
            Err(e) => log::error!("platform runtime: exited with error: {e}"),
        }

        crate::platform_early_log!("host.run.exit");
        result
    }

    pub(crate) fn submit_platform_job(
        &mut self,
        request: PlatformHostJobRequestV1,
        callback: PlatformHostJobCallbackV1,
        callback_user_data: usize,
    ) -> PlatformHostJobTicketV1 {
        if callback.is_null() {
            return PlatformHostJobTicketV1 {
                accepted: false,
                status: RString::from("rejected"),
                detail: RString::from("platform job callback was null"),
                ..Default::default()
            };
        }

        let callback_addr = callback.callback_addr;
        let label = leak_job_label(request.label.as_str(), "platform.job");
        let source = leak_job_label(request.source.as_str(), "engine.platform");
        let owner = leak_job_label(request.owner.as_str(), "platform-runtime");
        let category = leak_job_label(request.category.as_str(), "platform");
        let mut job = JobRequest::new(label)
            .with_source(source)
            .with_owner(owner)
            .with_category(category)
            .with_lane(platform_job_lane(request.lane.as_str()))
            .with_priority(platform_job_priority(request.priority.as_str()))
            .pausable(false)
            .cancellable(request.can_cancel);
        if !request.task_id.trim().is_empty() {
            job = job.with_task_id(request.task_id.to_string());
        }

        let ticket = self.engine.job_system().submit_controlled(job, move |control| {
            control.publish_progress(0.0, "Platform job entered", "Platform provider callback is running on engine.jobs.");
            // SAFETY: platform providers build this handle with
            // `PlatformHostJobCallbackV1::from_fn`. The handle crosses the ABI
            // as a plain address because `abi_stable` does not derive
            // `StableAbi` for function-pointer parameters nested inside another
            // function-pointer signature. The callback is executed once by the
            // submitted engine.jobs task.
            let callback_fn: extern "C" fn(usize) -> abi_stable::std_types::RResult<(), RString> =
                unsafe { std::mem::transmute(callback_addr) };
            let result = callback_fn(callback_user_data);
            match result {
                abi_stable::std_types::RResult::ROk(()) => {
                    control.publish_progress(1.0, "Platform job completed", "Platform provider callback completed normally.");
                }
                abi_stable::std_types::RResult::RErr(e) => {
                    control.publish_progress(1.0, "Platform job failed", e.to_string());
                }
            }
        });

        PlatformHostJobTicketV1 {
            accepted: true,
            job_id: RString::from(ticket.task_id()),
            status: RString::from("scheduled"),
            detail: RString::from("Platform job submitted to engine.jobs."),
        }
    }

    fn shutdown_engine_once(&mut self, origin: &'static str) {
        if self.shutting_down || matches!(self.engine.run_state(), EngineRunState::Stopped | EngineRunState::Faulted) {
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
        newengine_time_runtime::register_time_gateway_best_effort();
        register_jobs_gateway_service_best_effort(self.engine.job_system(), self.engine.events().clone());
        register_platform_window_service_best_effort(ready);
        let (display, window) = native_to_raw_handles(ready.handles)?;

        self.engine.resources_mut().insert(WindowHandles { window, display });
        self.engine.resources_mut().insert(WindowInitSize {
            width: ready.surface.width,
            height: ready.surface.height,
        });


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

        if focused {
            // Focus regain is a platform edge: winit/OS may have dropped the grab while
            // the engine-side render controller still believes the cursor is captured.
            // Re-apply the last canonical cursor state even when no module publishes a
            // new Cursor event on this exact frame.
            self.force_cursor_reapply = true;
        }
        Ok(())
    }

    pub(crate) fn on_close_requested(&mut self) -> EngineResult<()> {
        if self.close_requested {
            log::debug!("platform runtime: close requested ignored; shutdown already requested");
            return Ok(());
        }

        self.close_requested = true;
        log::info!("platform runtime: close requested; native window exit will be performed before engine teardown");
        self.engine
            .emit(HostEvent::Window(WindowHostEvent::CloseRequested))?;
        self.engine.request_exit()?;
        Ok(())
    }

    pub(crate) fn step(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
        let step = if self.close_requested {
            PlatformStepResultV1 {
                exit_requested: true,
                ..PlatformStepResultV1::default()
            }
        } else if self.runtime_soft_degraded_error.is_some() {
            self.runtime_soft_degraded_step_result()
        } else if self.fatal_bootstrap_error.is_some() {
            self.fatal_bootstrap_step_result()
        } else {
            let bootstrap_active = self.bootstrap_stage != RuntimeBootstrapStage::Running || !self.window_ready_emitted;
            let result = if bootstrap_active {
                self.step_bootstrap()
            } else {
                self.step_running(dt_sec)
            };

            match result {
                Ok(step) => step,
                Err(e) if bootstrap_active => {
                    let message = e.to_string();
                    log::error!("platform runtime bootstrap: fatal startup error: {message}");
                    self.fatal_bootstrap_error = Some(message);
                    self.fatal_bootstrap_step_result()
                }
                Err(e) => return Err(e),
            }
        };

        Ok(step)
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
                        self.refresh_ui_provider_binding("engine-plugins-loaded");
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
                    "Finalizing runtime handoff...",
                    "Player control and world presentation remain locked until the scene launch gate opens.",
                    0.97,
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
        self.ui_frame_index = self.ui_frame_index.wrapping_add(1);
        let ui_frame_index = self.ui_frame_index;
        let input_frame = poll_input_frame();
        if let Some(telemetry) = self
            .engine
            .resources
            .get::<newengine_ui_api::UiRuntimeDebugOverlayTelemetry>()
            .cloned()
        {
            crate::platform_runtime::ui_gateway_frame::publish_debug_overlay_telemetry(&telemetry);
        }
        // Modal UI state is produced inside engine.step() by render_controller and
        // requires same-frame refresh. Do not publish/request the previous frame's
        // primary UI node here: that duplicates engine.ui work and forces stale UI traffic
        // before the real modal owner has updated animation/navigation state.

        if let Some(input) = input_frame.clone() {
            self.engine.resources_mut().insert::<UiInputFrame>(input);
        } else {
            let _ = self.engine.resources_mut().remove::<UiInputFrame>();
        }

        if let Some(status) = self.engine.resources.get::<SceneLaunchStatus>().cloned() {
            if status.active && matches!(self.ui_selection.active(), UiProviderKind::Plugin { .. }) {
                let overlay = self.scene_launch_overlay(&status);
                crate::platform_runtime::ui_gateway_frame::publish_loading_overlay(
                    &overlay,
                    self.ui_provider_binding(),
                    ui_frame_index,
                );
            }
        }

        let provider_ui_active = matches!(self.ui_selection.active(), UiProviderKind::Plugin { .. });
        let debug_overlay_active = self
            .engine
            .resources
            .get::<newengine_ui_api::UiRuntimeDebugOverlayTelemetry>()
            .is_some();
        let scene_launch_active = self
            .engine
            .resources
            .get::<SceneLaunchStatus>()
            .map(|status| status.active)
            .unwrap_or(false);

        // Provider UI is a persistent overlay contract, not only a debug-overlay side effect.
        // The 1000-fps hot-path pass accidentally skipped engine.ui after launch unless
        // runtime-debug telemetry was enabled, so the gameplay HUD vanished and the frame graph
        // legitimately collapsed to `ui=none`. Keep UI visible by using a cached provider draw
        // list for idle gameplay, and refresh it only when state can change.
        let provider_ui_needed = self.ui_build.is_some() || debug_overlay_active || scene_launch_active;
        let provider_gameplay_hud = provider_ui_active && !self.minimized && self.surface.width > 0 && self.surface.height > 0;
        let provider_ui_refresh = provider_ui_needed
            || self.cached_provider_ui_draw.is_none()
            || ui_frame_index <= 4
            || ui_frame_index % 30 == 1;

        let mut ui_draw = if provider_ui_active && (provider_ui_needed || provider_gameplay_hud) {
            if provider_ui_refresh {
                match crate::platform_runtime::ui_gateway_frame::request_ui_draw_list(
                    ui_frame_index,
                    dt_sec,
                    [self.surface.width, self.surface.height],
                    self.surface.pixels_per_point,
                )? {
                    Some(draw_list) => {
                        let mut cached = draw_list.clone();
                        cached.texture_delta.clear();
                        self.cached_provider_ui_draw = Some(cached);
                        Some(draw_list)
                    }
                    None if provider_ui_needed => {
                        self.cached_provider_ui_draw = None;
                        None
                    }
                    None => self.cached_provider_ui_draw.clone(),
                }
            } else {
                self.cached_provider_ui_draw.clone()
            }
        } else {
            self.cached_provider_ui_draw = None;
            None
        };

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
            if !out.draw_list.mesh.vertices.is_empty() || !out.draw_list.mesh.indices.is_empty() {
                ui_draw = Some(out.draw_list);
            }
        }

        if let Some(draw_list) = ui_draw {
            self.engine.resources_mut().insert(draw_list);
        } else {
            let _ = self.engine.resources_mut().remove::<newengine_ui_api::UiDrawList>();
        }

        match self.engine.step() {
            Ok(()) => {
                // ModuleCtx::request_exit() may be raised during the frame and
                // converted into the shared shutdown token after Engine::step().
                // Do not wait for a later redraw/input event: return an explicit
                // platform exit now so winit tears down the window and engine.shutdown
                // runs, allowing profiler plugins to flush final reports.
                if self.engine.shutdown_token().is_requested() {
                    log::info!("platform runtime: shutdown requested by engine module; requesting native exit");
                    return Ok(PlatformStepResultV1 {
                        exit_requested: true,
                        ..PlatformStepResultV1::default()
                    });
                }

                if let Some(status) = self.engine.resources.get::<SceneLaunchStatus>().cloned() {
                    if status.active {
                        return Ok(self.scene_launch_step_result(&status));
                    }
                    if matches!(self.ui_selection.active(), UiProviderKind::Plugin { .. }) {
                        crate::platform_runtime::ui_gateway_frame::publish_loading_overlay_inactive(ui_frame_index);
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
            Err(e) => {
                let message = e.to_string();
                log::error!("platform runtime: engine.step failed in running state; entering soft degradation instead of exiting: {message}");
                Ok(self.enter_runtime_soft_degraded_step("engine.step", message))
            }
        }
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
        log::error!(
            "platform runtime: soft degradation activated origin='{}' message='{}'",
            origin,
            message
        );
        self.runtime_soft_degraded_origin = Some(origin);
        self.runtime_soft_degraded_error = Some(message.clone());
        self.engine.resources_mut().insert(RenderBackendStatus::degraded(origin, message));
        self.runtime_soft_degraded_step_result()
    }

    fn runtime_soft_degraded_step_result(&mut self) -> PlatformStepResultV1 {
        self.runtime_soft_degraded_frames = self.runtime_soft_degraded_frames.wrapping_add(1);
        let origin = self.runtime_soft_degraded_origin.unwrap_or("runtime");
        let message = self
            .runtime_soft_degraded_error
            .as_deref()
            .unwrap_or("Runtime entered recovery mode without a diagnostic message.");
        if self.runtime_soft_degraded_frames == 1 || self.runtime_soft_degraded_frames % 120 == 1 {
            log::error!(
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
        .with_subsystems(self.bootstrap_subsystems());
        self.loading_overlay_step_result(&overlay, self.runtime_soft_degraded_frames as u32)
    }


    fn scene_launch_step_result(&mut self, status: &SceneLaunchStatus) -> PlatformStepResultV1 {
        self.bootstrap_spinner_phase = self.bootstrap_spinner_phase.wrapping_add(1);
        let overlay = self.scene_launch_overlay(status);

        if matches!(self.ui_selection.active(), UiProviderKind::Plugin { .. }) {
            crate::platform_runtime::ui_gateway_frame::publish_loading_overlay(
                &overlay,
                self.ui_provider_binding(),
                self.bootstrap_spinner_phase as u64,
            );
            if self.bootstrap_spinner_phase % 120 == 1 {
                log::info!(
                    "platform loading overlay: source=engine.ui provider='{}' platform_overlay_state_only=true",
                    self.ui_provider_binding().id()
                );
            }
            return overlay_to_step_result_with_provider(&overlay, self.bootstrap_spinner_phase, self.ui_provider_binding());
        }

        if self.bootstrap_spinner_phase % 120 == 1 {
            log::warn!(
                "platform loading overlay: engine.ui provider unavailable; no platform-native UI renderer will be used"
            );
        }
        overlay_to_step_result_with_provider(&overlay, self.bootstrap_spinner_phase, UiProviderBinding::None)
    }

    fn scene_launch_overlay(&self, status: &SceneLaunchStatus) -> ScreenOverlayStatus {
        bootstrap_loading_with_subsystems(
            status.title.as_str(),
            status.status.as_str(),
            status.detail.as_str(),
            status.progress_01,
            self.scene_launch_subsystems(status),
        )
    }


    fn degraded_backend_step_result(&self, status: &RenderBackendStatus) -> PlatformStepResultV1 {
        match overlay_from_render_backend_status(status) {
            Some(overlay) => self.overlay_step_result(&overlay, 0),
            None => PlatformStepResultV1::default(),
        }
    }

    fn overlay_step_result(&self, overlay: &ScreenOverlayStatus, spinner_phase: u32) -> PlatformStepResultV1 {
        overlay_to_step_result_with_provider(overlay, spinner_phase, self.overlay_provider_binding())
    }

    fn loading_overlay_step_result(&self, overlay: &ScreenOverlayStatus, spinner_phase: u32) -> PlatformStepResultV1 {
        // Bootstrap loading remains data-only at the platform boundary. Visual UI
        // must be rendered through engine.ui when a provider route exists; otherwise
        // the platform keeps stepping startup and logs diagnostics without drawing.
        if newengine_core::has_engine_gateway_route(newengine_ui_api::ENGINE_UI_SERVICE_ID) {
            crate::platform_runtime::ui_gateway_frame::publish_loading_overlay(
                overlay,
                self.overlay_provider_binding(),
                spinner_phase as u64,
            );
        } else if spinner_phase % 120 == 1 {
            log::warn!(
                "bootstrap loading overlay: engine.ui unavailable; no special/native UI renderer will be used"
            );
        }
        overlay_to_step_result_with_provider(overlay, spinner_phase, self.overlay_provider_binding())
    }

    fn ui_provider_binding(&self) -> UiProviderBinding {
        self.ui_selection.binding()
    }

    fn overlay_provider_binding(&self) -> UiProviderBinding {
        match self.ui_selection.active() {
            UiProviderKind::Plugin { .. } => self.ui_provider_binding(),
            UiProviderKind::Null => UiProviderBinding::None,
        }
    }

    fn refresh_ui_provider_binding(&mut self, origin: &'static str) {
        let Some(next) = self.ui_selection.refresh(origin) else {
            return;
        };

        self.ui = create_provider(UiProviderOptions { kind: next });
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
            return self.loading_overlay_step_result(&overlay, self.bootstrap_spinner_phase);
        }

        let status = self.bootstrap_overlay.status.as_str();
        let detail = self.bootstrap_overlay.detail.as_str();
        let subsystems = self.bootstrap_subsystems();

        let overlay = bootstrap_loading_with_subsystems(
            self.bootstrap_overlay.title.as_str(),
            status,
            detail,
            self.bootstrap_overlay.progress_01,
            subsystems,
        );

        self.loading_overlay_step_result(&overlay, self.bootstrap_spinner_phase)
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

        self.loading_overlay_step_result(&overlay, self.bootstrap_spinner_phase)
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

    fn render_backend_label(&self) -> String {
        self.engine
            .resources
            .get::<ResolvedRenderBackendConfig>()
            .map(|resolved| render_backend_label_from_id(resolved.backend_id.as_str()))
            .unwrap_or_else(|| "WAIT".to_owned())
    }

    pub(crate) fn poll_cursor_state(&mut self) -> PlatformCursorPollV1 {
        let mut last: Option<CursorState> = None;

        self.host_events.drain(|ev| {
            if let HostEvent::Window(WindowHostEvent::Cursor(state)) = ev.as_ref() {
                last = Some(*state);
            }
        });

        if let Some(state) = last {
            self.last_platform_cursor = state;
            self.force_cursor_reapply = false;
            return cursor_poll_from_state(state);
        }

        if self.force_cursor_reapply {
            self.force_cursor_reapply = false;
            return cursor_poll_from_state(self.last_platform_cursor);
        }

        PlatformCursorPollV1::default()
    }
}

#[inline]
fn cursor_poll_from_state(state: CursorState) -> PlatformCursorPollV1 {
    PlatformCursorPollV1 {
        has_value: true,
        state: PlatformCursorStateV1 {
            visible: state.visible,
            grab: match state.grab {
                CursorGrabMode::None => PlatformCursorGrabModeV1::None,
                CursorGrabMode::Confined => PlatformCursorGrabModeV1::Confined,
                CursorGrabMode::Locked => PlatformCursorGrabModeV1::Locked,
            },
        },
    }
}

fn render_backend_label_from_id(id: &str) -> String {
    id.rsplit('.')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(id)
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_uppercase())
        .collect::<String>()
}

fn platform_job_lane(value: &str) -> JobLane {
    match value.trim().to_ascii_lowercase().as_str() {
        "simulation" => JobLane::Simulation,
        "render-prep" | "render_prep" | "renderprep" => JobLane::RenderPrep,
        "streaming" => JobLane::Streaming,
        "asset-io" | "asset_io" | "asset" => JobLane::AssetIo,
        "plugin" | "plugins" => JobLane::Plugin,
        "background" | "bg" => JobLane::Background,
        _ => JobLane::Background,
    }
}

fn platform_job_priority(value: &str) -> JobPriority {
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" => JobPriority::Critical,
        "interactive" => JobPriority::Interactive,
        "normal" => JobPriority::Normal,
        "background" | "bg" => JobPriority::Background,
        _ => JobPriority::Normal,
    }
}

fn leak_job_label(value: &str, fallback: &'static str) -> &'static str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return fallback;
    }
    Box::leak(trimmed.to_owned().into_boxed_str())
}
