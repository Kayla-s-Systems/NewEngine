use abi_stable::std_types::RString;
use libloading::Library;
use newengine_core::events::EventSub;
use newengine_core::host_events::{
    CursorState, HostEvent, WindowHandles, WindowHostEvent, WindowInitSize,
};
use newengine_core::{Engine, EngineError, EngineResult};
use newengine_platform_api::{
    PlatformDisplayConfigV1, PlatformHostApiV1, PlatformHostJobCallbackV1,
    PlatformHostTaskRequestV1, PlatformHostTaskTicketV1, PlatformRuntimeDescriptorV1Fn,
    PlatformRuntimeRunFnV1, PlatformStepResultV1, PlatformSurfaceMetricsV1, PlatformWindowReadyV1,
    PLATFORM_RUNTIME_DESCRIPTOR_V1_SYMBOL_BYTES_NUL,
};
use newengine_plugin_api::PluginInfo;
use newengine_ui::{create_provider, UiBuildFn, UiProvider, UiProviderKind, UiProviderOptions};
use newengine_ui_api::UiLayerDomain;
use std::path::Path;

use crate::platform_runtime::bootstrap_overlay::{
    RuntimeBootstrapOverlayState, RuntimeBootstrapStage,
};
use crate::platform_runtime::callbacks::{
    host_on_close_requested_v1, host_on_window_focused_v1, host_on_window_ready_v1,
    host_on_window_resized_v1, host_poll_cursor_state_v1, host_step_v1, host_submit_job_v1,
};
use crate::platform_runtime::constants::PLATFORM_RUNTIME_SYMBOL;
use crate::platform_runtime::handles::{native_to_raw_handles, raw_to_native_handles};
use crate::platform_runtime::screen_profile::ScreenProfileRuntimeState;
use crate::platform_runtime::shutdown_watchdog::ShutdownWatchdog;
use crate::platform_runtime::snapshot_service::{
    register_platform_window_service_best_effort, update_platform_window_snapshot,
};
use crate::platform_runtime::types::ResolvedPlatformRuntimeConfig;
use crate::platform_runtime::ui_gateway_frame::UiGatewayFramePolicy;
use crate::platform_runtime::ui_layer_cache::RetainedUiLayerCache;
use crate::platform_runtime::ui_provider_selection::{
    log_ui_provider_selection, UiProviderSelection,
};
use newengine_runtime_host::register_threading_gateway_service_best_effort;

#[path = "runtime_host_parts/mod.rs"]
mod runtime_host_parts;

pub struct HostPlatformRuntime {
    engine: Engine<()>,
    ui: Box<dyn UiProvider>,
    ui_build: Option<Box<dyn UiBuildFn>>,
    ui_selection: UiProviderSelection,
    screen_profile: ScreenProfileRuntimeState,
    host_events: EventSub<HostEvent>,
    surface: PlatformSurfaceMetricsV1,
    display: PlatformDisplayConfigV1,
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
    /// Retained UI presentation domains have independent lifecycle/cache ownership.
    /// World rendering never owns or clears the game viewport cache.
    system_ui_cache: RetainedUiLayerCache,
    game_ui_cache: RetainedUiLayerCache,
    editor_ui_cache: RetainedUiLayerCache,
    debug_ui_cache: RetainedUiLayerCache,
    ui_frame_policy: UiGatewayFramePolicy,
    runtime_bootstrap_overlay_enabled: bool,
}

impl HostPlatformRuntime {
    pub fn new(
        mut engine: Engine<()>,
        ui_kind: UiProviderKind,
        ui_build: Option<Box<dyn UiBuildFn>>,
    ) -> Self {
        let host_events = engine.events().subscribe::<HostEvent>();
        let ui_selection = UiProviderSelection::new(ui_kind);
        log_ui_provider_selection("initial", ui_selection.active());
        let active_ui_kind = ui_selection.active().clone();
        if let Err(error) = newengine_ui_notify_runtime::install_ui_notify_runtime(&mut engine) {
            newengine_ulog_api::ulog::warn!(
                "windowed host: engine.ui.notify runtime unavailable; notifications degrade to no-op: {}",
                error
            );
        }
        let screen_profile = ScreenProfileRuntimeState::load();
        screen_profile.install_initial_resources(engine.resources_mut());

        Self {
            engine,
            ui: create_provider(UiProviderOptions {
                kind: active_ui_kind,
            }),
            ui_build,
            ui_selection,
            screen_profile,
            host_events,
            surface: PlatformSurfaceMetricsV1::default(),
            display: PlatformDisplayConfigV1::default(),
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
            system_ui_cache: RetainedUiLayerCache::new(UiLayerDomain::System),
            game_ui_cache: RetainedUiLayerCache::new(UiLayerDomain::GameViewport),
            editor_ui_cache: RetainedUiLayerCache::new(UiLayerDomain::Editor),
            debug_ui_cache: RetainedUiLayerCache::new(UiLayerDomain::Debug),
            ui_frame_policy: UiGatewayFramePolicy::from_startup_config(
                newengine_core::startup::last_startup_config(),
            ),
            runtime_bootstrap_overlay_enabled:
                crate::platform_runtime::config::runtime_bootstrap_overlay_enabled(),
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

        crate::platform_early_log!("host.run.enter runtime_path='{}'", runtime_path.display());
        newengine_ulog_api::ulog::info!("platform runtime: loading '{}'", runtime_path.display());

        crate::platform_early_log!("host.dll.load.begin path='{}'", runtime_path.display());
        let lib = unsafe { Library::new(runtime_path) }.map_err(|e| {
            crate::platform_early_log!("host.dll.load.err error='{}'", e);
            EngineError::other(format!("platform runtime load failed: {e}"))
        })?;
        crate::platform_early_log!("host.dll.load.ok path='{}'", runtime_path.display());

        let planned_descriptor =
            crate::platform_runtime::discovery::try_read_runtime_descriptor(runtime_path)
                .ok_or_else(|| {
                    EngineError::other(format!(
                        "platform runtime verified discovery metadata disappeared for '{}'",
                        runtime_path.display()
                    ))
                })?;
        let live_descriptor_fn: libloading::Symbol<PlatformRuntimeDescriptorV1Fn> =
            unsafe { lib.get(PLATFORM_RUNTIME_DESCRIPTOR_V1_SYMBOL_BYTES_NUL) }.map_err(|e| {
                EngineError::other(format!(
                    "platform runtime descriptor symbol missing after selection: {e}"
                ))
            })?;
        let live_descriptor = live_descriptor_fn();
        let mut planned_tags = planned_descriptor
            .system_tags
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>();
        let mut live_tags = live_descriptor
            .system_tags
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>();
        planned_tags.sort();
        planned_tags.dedup();
        live_tags.sort();
        live_tags.dedup();
        if live_descriptor.schema_version
            != newengine_platform_api::PlatformRuntimeDescriptorV1::SCHEMA_VERSION
            || live_descriptor.id != planned_descriptor.id
            || live_descriptor.name != planned_descriptor.name
            || live_descriptor.version != planned_descriptor.version
            || live_descriptor.backend_priority != planned_descriptor.backend_priority
            || live_tags != planned_tags
        {
            return Err(EngineError::other(format!(
                "platform runtime live descriptor does not match frozen discovery metadata path='{}'",
                runtime_path.display()
            )));
        }
        crate::platform_early_log!(
            "host.dll.metadata.verify.ok path='{}'",
            runtime_path.display()
        );

        crate::platform_early_log!(
            "host.symbol.resolve.begin symbol='{}'",
            "newengine_platform_runtime_run_v1"
        );
        let run: libloading::Symbol<PlatformRuntimeRunFnV1> =
            unsafe { lib.get(PLATFORM_RUNTIME_SYMBOL) }.map_err(|e| {
                crate::platform_early_log!("host.symbol.resolve.err error='{}'", e);
                EngineError::other(format!("platform runtime symbol missing: {e}"))
            })?;
        crate::platform_early_log!(
            "host.symbol.resolve.ok symbol='{}'",
            "newengine_platform_runtime_run_v1"
        );

        newengine_plugin_host::register_external_runtime_plugin(
            runtime_path.to_path_buf(),
            info,
            resolved.descriptor.clone(),
            "running",
        )
        .map_err(EngineError::other)?;

        newengine_ulog_api::ulog::info!(
            "platform runtime: entry resolved symbol='{}' title='{}' size={}x{}",
            "newengine_platform_runtime_run_v1",
            config.title,
            config.width,
            config.height
        );

        // Null routes are real visible providers with the lowest origin tier.
        // Concrete plugins shadow them automatically; missing domains degrade
        // instead of crashing or constructing hidden fallbacks.

        // engine.threading is available before the platform provider starts its
        // native bootstrap surface. Platform plugins must submit bootstrap work
        // through this callback instead of creating hidden threads.
        register_threading_gateway_service_best_effort(
            self.engine.thread_pool(),
            self.engine.events().clone(),
        );

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
        newengine_ulog_api::ulog::info!(
            "platform runtime: native runtime returned status={} close_requested={} engine_shutdown_requested={} started={} window_ready_emitted={} bootstrap_stage={:?} ui_frames={} surface={}x{} minimized={} degraded={} fatal_bootstrap={} reason='platform-runtime-return'",
            if result.is_ok() { "ok" } else { "err" },
            self.close_requested,
            self.engine.shutdown_token().is_requested(),
            self.started,
            self.window_ready_emitted,
            self.bootstrap_stage,
            self.ui_frame_index,
            self.surface.width,
            self.surface.height,
            self.minimized,
            self.runtime_soft_degraded_error.is_some(),
            self.fatal_bootstrap_error.is_some(),
        );

        let shutdown_exit_code = if result.is_ok() { 0 } else { 1 };
        let shutdown_watchdog = ShutdownWatchdog::arm(
            self.engine.thread_pool(),
            "platform runtime returned",
            shutdown_exit_code,
        );

        self.shutdown_engine_once("platform runtime returned");

        newengine_plugin_host::host_context::unregister_by_owner(&resolved.plugin_id);
        shutdown_watchdog.complete();

        match &result {
            Ok(()) => newengine_ulog_api::ulog::info!("platform runtime: exited cleanly"),
            Err(e) => newengine_ulog_api::ulog::error!("platform runtime: exited with error: {e}"),
        }

        crate::platform_early_log!("host.run.exit");
        result
    }

    pub(crate) fn submit_platform_job(
        &mut self,
        request: PlatformHostTaskRequestV1,
        callback: PlatformHostJobCallbackV1,
        callback_user_data: usize,
    ) -> PlatformHostTaskTicketV1 {
        submit_platform_task(
            &self.engine.thread_pool(),
            request,
            callback,
            callback_user_data,
        )
    }

    pub(crate) fn on_window_ready(&mut self, ready: PlatformWindowReadyV1) -> EngineResult<()> {
        newengine_ulog_api::ulog::info!(
            "platform runtime: window ready backend={:?} size={}x{} ppp={:.3} vsync={} refresh_millihz={} mode={:?}",
            ready.handles.backend,
            ready.surface.width,
            ready.surface.height,
            ready.surface.pixels_per_point,
            ready.display.vsync,
            ready.display.refresh_rate_millihz,
            ready.display.window_mode
        );

        self.surface = ready.surface;
        self.display = ready.display;
        newengine_time_runtime::register_time_gateway_best_effort();
        newengine_schema_runtime::register_schema_gateway_best_effort();
        register_threading_gateway_service_best_effort(
            self.engine.thread_pool(),
            self.engine.events().clone(),
        );
        register_platform_window_service_best_effort(ready);
        let (display, window) = native_to_raw_handles(ready.handles)?;

        self.engine
            .resources_mut()
            .insert(WindowHandles { window, display });
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
        newengine_ulog_api::ulog::info!(
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
        newengine_ulog_api::ulog::debug!(
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
                // Resize callbacks do not carry a new display policy; keep the
                // presentation policy selected during the initial window-ready event.
                display: self.display,
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
                .emit(HostEvent::Window(WindowHostEvent::MinimizedChanged(
                    minimized,
                )))?;
        }

        self.engine
            .emit(HostEvent::Window(WindowHostEvent::Resized {
                width: metrics.width,
                height: metrics.height,
            }))?;
        Ok(())
    }

    pub(crate) fn on_window_focused(&mut self, focused: bool) -> EngineResult<()> {
        newengine_ulog_api::ulog::debug!("platform runtime: focused={focused}");
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
            newengine_ulog_api::ulog::debug!(
                "platform runtime: close requested ignored; shutdown already requested"
            );
            return Ok(());
        }

        self.close_requested = true;
        newengine_ulog_api::ulog::info!("platform runtime: close requested; native window exit will be performed before engine teardown");
        self.engine
            .emit(HostEvent::Window(WindowHostEvent::CloseRequested))?;
        self.engine.request_exit()?;
        Ok(())
    }

    pub(crate) fn step(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
        let step = if self.close_requested {
            newengine_ulog_api::ulog::info!(
                "platform runtime: step requested native exit reason='close_requested' ui_frames={} started={} bootstrap_stage={:?}",
                self.ui_frame_index,
                self.started,
                self.bootstrap_stage,
            );
            PlatformStepResultV1 {
                exit_requested: true,
                ..PlatformStepResultV1::default()
            }
        } else if self.runtime_soft_degraded_error.is_some() {
            self.runtime_soft_degraded_step_result()
        } else if self.fatal_bootstrap_error.is_some() {
            self.fatal_bootstrap_step_result()
        } else {
            let bootstrap_active = self.bootstrap_stage != RuntimeBootstrapStage::Running
                || !self.window_ready_emitted;
            let result = if bootstrap_active {
                self.step_bootstrap()
            } else {
                self.step_running(dt_sec)
            };

            match result {
                Ok(step) => step,
                Err(e) if bootstrap_active => {
                    let message = e.to_string();
                    newengine_ulog_api::ulog::error!(
                        "platform runtime bootstrap: fatal startup error: {message}"
                    );
                    self.fatal_bootstrap_error = Some(message);
                    self.fatal_bootstrap_step_result()
                }
                Err(e) => return Err(e),
            }
        };

        Ok(step)
    }
}
use runtime_host_parts::platform_task_submission::submit_platform_task;
