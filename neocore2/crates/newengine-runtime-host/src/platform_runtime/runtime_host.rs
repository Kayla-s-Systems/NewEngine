use std::path::Path;

use abi_stable::std_types::RString;
use libloading::Library;
use newengine_core::events::EventSub;
use newengine_core::host_events::{
    CursorGrabMode, CursorState, HostEvent, WindowHandles, WindowHostEvent, WindowInitSize,
};
use newengine_core::{Engine, EngineError, EngineResult};
use newengine_platform_api::{
    PlatformCursorGrabModeV1, PlatformCursorPollV1, PlatformCursorStateV1, PlatformHostApiV1,
    PlatformRuntimeRunFnV1, PlatformStepResultV1, PlatformSurfaceMetricsV1,
    PlatformWindowReadyV1,
};
use newengine_plugin_api::PluginInfo;
use newengine_ui::{
    create_provider, UiBuildFn, UiFrameDesc, UiProvider, UiProviderKind, UiProviderOptions,
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

pub struct HostPlatformRuntime {
    engine: Engine<()>,
    ui: Box<dyn UiProvider>,
    ui_build: Option<Box<dyn UiBuildFn>>,
    host_events: EventSub<HostEvent>,
    surface: PlatformSurfaceMetricsV1,
    minimized: bool,
    started: bool,
}

impl HostPlatformRuntime {
    pub fn new(
        engine: Engine<()>,
        ui_kind: UiProviderKind,
        ui_build: Option<Box<dyn UiBuildFn>>,
    ) -> Self {
        let host_events = engine.events().subscribe::<HostEvent>();
        Self {
            engine,
            ui: create_provider(UiProviderOptions { kind: ui_kind }),
            ui_build,
            host_events,
            surface: PlatformSurfaceMetricsV1::default(),
            minimized: false,
            started: false,
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

        if self.started {
            newengine_core::crash::record_breadcrumb("platform runtime: engine.shutdown begin");
            let _ = self.engine.shutdown();
            newengine_core::crash::record_breadcrumb("platform runtime: engine.shutdown completed");
        }

        newengine_plugin_host::host_context::unregister_by_owner(&resolved.plugin_id);

        match &result {
            Ok(()) => log::info!("platform runtime: exited cleanly"),
            Err(e) => log::error!("platform runtime: exited with error: {e}"),
        }

        result
    }

    pub(crate) fn on_window_ready(
        &mut self,
        ready: PlatformWindowReadyV1,
    ) -> EngineResult<()> {
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

        if !self.started {
            if let Some(startup) = newengine_core::startup::last_startup_config() {
                std::env::set_var("NEWENGINE_RENDER_BACKEND", startup.render_backend.as_str());
            }

            match self.engine.load_engine_plugins_once() {
                Ok(count) => {
                    log::info!(
                        "platform runtime: engine plugins init completed loaded_count={}",
                        count
                    );
                }
                Err(e) => {
                    log::error!("platform runtime: engine plugins init failed: {}", e);
                    return Err(e);
                }
            }

            match self.engine.start() {
                Ok(()) => {
                    self.started = true;
                    log::info!("platform runtime: engine.start completed");
                }
                Err(e) => {
                    log::error!("platform runtime: engine.start failed: {}", e);
                    return Err(e);
                }
            }
        }

        match self.engine.emit(HostEvent::Window(WindowHostEvent::Ready {
            width: ready.surface.width,
            height: ready.surface.height,
        })) {
            Ok(()) => {}
            Err(e) => {
                log::error!("platform runtime: emit WindowReady failed: {}", e);
                return Err(e);
            }
        }

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
        Ok(())
    }

    pub(crate) fn step(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
        if let Some(build) = self.ui_build.as_deref_mut() {
            let mut desc = UiFrameDesc::new(dt_sec).with_surface(
                self.surface.width,
                self.surface.height,
                self.surface.pixels_per_point,
            );

            if let Some(input) = poll_input_frame() {
                desc = desc.with_input(input);
            }

            let out = self.ui.run_frame(&(), desc, build);
            self.engine.resources_mut().insert(out.draw_list);
        }

        match self.engine.step() {
            Ok(()) => Ok(PlatformStepResultV1 {
                exit_requested: false,
            }),
            Err(EngineError::ExitRequested) => Ok(PlatformStepResultV1 {
                exit_requested: true,
            }),
            Err(e) => Err(e),
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