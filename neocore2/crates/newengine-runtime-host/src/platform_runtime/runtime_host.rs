use std::path::Path;

use abi_stable::std_types::RString;
use libloading::Library;
use newengine_core::events::EventSub;
use newengine_core::render::{RenderBackendStatus, SceneLaunchStatus};
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
use newengine_system_runtime::{
    overlay_from_render_backend_status, overlay_to_step_result,
    startup_status_mapper::{bootstrap_loading, runtime_ready},
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
use crate::platform_runtime::constants::PLATFORM_RUNTIME_SYMBOL;
use crate::platform_runtime::handles::{native_to_raw_handles, raw_to_native_handles};
use crate::platform_runtime::snapshot_service::{
    register_platform_window_service_best_effort, update_platform_window_snapshot,
};
use crate::platform_runtime::types::ResolvedPlatformRuntimeConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeBootstrapStage {
    AwaitingWindow,
    AnnounceLoadEnginePlugins,
    LoadEnginePlugins,
    AnnounceStartEngine,
    StartEngine,
    AnnounceEnterRuntime,
    EmitWindowReady,
    ReadyOverlay,
    Running,
}

#[derive(Debug, Clone)]
struct RuntimeBootstrapOverlayState {
    title: String,
    status: String,
    detail: String,
    progress_01: f32,
}

impl Default for RuntimeBootstrapOverlayState {
    #[inline]
    fn default() -> Self {
        Self {
            title: "NEWENGINE // BOOTSTRAP".to_owned(),
            status: "Waiting for platform window...".to_owned(),
            detail: "The runtime shell is preparing the first visible frame.".to_owned(),
            progress_01: 0.0,
        }
    }
}

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
        if self.bootstrap_stage != RuntimeBootstrapStage::Running || !self.window_ready_emitted {
            return self.step_bootstrap();
        }

        self.step_running(dt_sec)
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
            RuntimeBootstrapStage::StartEngine => match self.engine.start() {
                Ok(()) => {
                    self.started = true;
                    log::info!("platform runtime: engine.start completed");
                    self.set_bootstrap_overlay(
                        "Engine runtime started.",
                        "Finalizing gated scene readiness and host window events.",
                        0.90,
                    );
                    self.bootstrap_stage = RuntimeBootstrapStage::AnnounceEnterRuntime;
                    Ok(self.loading_step_result())
                }
                Err(e) => {
                    log::error!("platform runtime: engine.start failed: {}", e);
                    Err(e)
                }
            },
            RuntimeBootstrapStage::AnnounceEnterRuntime => {
                self.set_bootstrap_overlay(
                    "Preparing playable world...",
                    "Native loading remains active while scene resources become resident.",
                    0.88,
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
                    0.90,
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
        let overlay = bootstrap_loading(
            status.title.as_str(),
            status.status.as_str(),
            status.detail.as_str(),
            status.progress_01,
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
        let next_progress = progress_01.clamp(0.0, 1.0);

        let changed = self.bootstrap_overlay.status != next_status
            || self.bootstrap_overlay.detail != next_detail
            || (self.bootstrap_overlay.progress_01 - next_progress).abs() > f32::EPSILON;

        self.bootstrap_overlay.status = next_status;
        self.bootstrap_overlay.detail = next_detail;
        self.bootstrap_overlay.progress_01 = next_progress;

        if changed {
            log::info!(
                "platform runtime bootstrap: overlay status='{}' detail='{}' progress={:.0}%",
                self.bootstrap_overlay.status,
                self.bootstrap_overlay.detail,
                self.bootstrap_overlay.progress_01 * 100.0
            );
        }
    }

    fn loading_step_result(&self) -> PlatformStepResultV1 {
        let status = self.bootstrap_overlay.status.as_str();
        let detail = self.bootstrap_overlay.detail.as_str();

        let overlay = if self.bootstrap_overlay.progress_01 >= 0.999 {
            runtime_ready(status, detail)
        } else {
            bootstrap_loading(
                self.bootstrap_overlay.title.as_str(),
                status,
                detail,
                self.bootstrap_overlay.progress_01,
            )
        };

        overlay_to_step_result(&overlay, self.bootstrap_spinner_phase)
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
