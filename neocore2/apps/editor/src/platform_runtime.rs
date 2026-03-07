#![forbid(unsafe_op_in_unsafe_fn)]

use std::num::NonZeroIsize;
use std::path::{Path, PathBuf};

use abi_stable::std_types::{RResult, RString};
use libloading::Library;
use newengine_core::events::EventSub;
use newengine_core::host_events::{
    CursorGrabMode, CursorState, HostEvent, WindowHandles, WindowHostEvent, WindowInitSize,
};
use newengine_core::{Engine, EngineError, EngineResult};
use newengine_platform_api::{
    NativeWindowBackendV1, NativeWindowHandlesV1, PlatformAppConfigV1, PlatformCursorGrabModeV1,
    PlatformCursorPollV1, PlatformCursorStateV1, PlatformHostApiV1, PlatformRuntimeRunFnV1,
    PlatformStepResultV1, PlatformSurfaceMetricsV1, PlatformWindowReadyV1,
};
use newengine_ui::{
    create_provider, UiBuildFn, UiFrameDesc, UiProvider, UiProviderKind, UiProviderOptions,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::platform_input::poll_input_frame;

const PLATFORM_RUNTIME_SYMBOL: &[u8] = b"newengine_platform_runtime_run_v1\0";

pub struct EditorPlatformRuntime {
    engine: Engine<()>,
    ui: Box<dyn UiProvider>,
    ui_build: Option<Box<dyn UiBuildFn>>,
    host_events: EventSub<HostEvent>,
    surface: PlatformSurfaceMetricsV1,
    minimized: bool,
    started: bool,
}

impl EditorPlatformRuntime {
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

    pub fn run(mut self, runtime_path: &Path, config: PlatformAppConfigV1) -> EngineResult<()> {
        log::info!("platform runtime: loading '{}'", runtime_path.display());

        let lib = unsafe { Library::new(runtime_path) }
            .map_err(|e| EngineError::other(format!("platform runtime load failed: {e}")))?;

        let run: libloading::Symbol<PlatformRuntimeRunFnV1> =
            unsafe { lib.get(PLATFORM_RUNTIME_SYMBOL) }
                .map_err(|e| EngineError::other(format!("platform runtime symbol missing: {e}")))?;

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

        let plugin_host = newengine_core::plugins::default_host_api();
        let result = unsafe { run(plugin_host, host, config) }
            .into_result()
            .map_err(|e| EngineError::other(e.to_string()));

        if self.started {
            let _ = self.engine.shutdown();
        }

        match &result {
            Ok(()) => log::info!("platform runtime: exited cleanly"),
            Err(e) => log::error!("platform runtime: exited with error: {e}"),
        }

        result
    }

    fn on_window_ready(&mut self, ready: PlatformWindowReadyV1) -> EngineResult<()> {
        log::info!(
            "platform runtime: window ready backend={:?} size={}x{} ppp={:.3}",
            ready.handles.backend,
            ready.surface.width,
            ready.surface.height,
            ready.surface.pixels_per_point
        );

        self.surface = ready.surface;
        let (display, window) = native_to_raw_handles(ready.handles)?;

        self.engine.resources_mut().insert(WindowHandles { window, display });
        self.engine.resources_mut().insert(WindowInitSize {
            width: ready.surface.width,
            height: ready.surface.height,
        });

        if !self.started {
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

    fn on_window_resized(&mut self, metrics: PlatformSurfaceMetricsV1) -> EngineResult<()> {
        log::debug!(
            "platform runtime: resized {}x{} ppp={:.3}",
            metrics.width,
            metrics.height,
            metrics.pixels_per_point
        );

        self.surface = metrics;
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

    fn on_window_focused(&mut self, focused: bool) -> EngineResult<()> {
        log::debug!("platform runtime: focused={focused}");
        self.engine
            .emit(HostEvent::Window(WindowHostEvent::Focused(focused)))?;
        Ok(())
    }

    fn on_close_requested(&mut self) -> EngineResult<()> {
        log::info!("platform runtime: close requested");
        self.engine
            .emit(HostEvent::Window(WindowHostEvent::CloseRequested))?;
        self.engine.request_exit()?;
        Ok(())
    }

    fn step(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
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

    fn poll_cursor_state(&mut self) -> PlatformCursorPollV1 {
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

fn runtime_state_mut<'a>(user_data: usize) -> &'a mut EditorPlatformRuntime {
    unsafe { &mut *(user_data as *mut EditorPlatformRuntime) }
}

extern "C" fn host_on_window_ready_v1(
    user_data: usize,
    ready: PlatformWindowReadyV1,
) -> RResult<(), RString> {
    match runtime_state_mut(user_data).on_window_ready(ready) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

extern "C" fn host_on_window_resized_v1(
    user_data: usize,
    metrics: PlatformSurfaceMetricsV1,
) -> RResult<(), RString> {
    match runtime_state_mut(user_data).on_window_resized(metrics) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

extern "C" fn host_on_window_focused_v1(
    user_data: usize,
    focused: bool,
) -> RResult<(), RString> {
    match runtime_state_mut(user_data).on_window_focused(focused) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

extern "C" fn host_on_close_requested_v1(user_data: usize) -> RResult<(), RString> {
    match runtime_state_mut(user_data).on_close_requested() {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

extern "C" fn host_step_v1(
    user_data: usize,
    dt_sec: f32,
) -> RResult<PlatformStepResultV1, RString> {
    match runtime_state_mut(user_data).step(dt_sec) {
        Ok(v) => RResult::ROk(v),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

extern "C" fn host_poll_cursor_state_v1(user_data: usize) -> PlatformCursorPollV1 {
    runtime_state_mut(user_data).poll_cursor_state()
}

#[cfg(target_os = "windows")]
fn native_to_raw_handles(
    handles: NativeWindowHandlesV1,
) -> EngineResult<(RawDisplayHandle, RawWindowHandle)> {
    use raw_window_handle::{Win32WindowHandle, WindowsDisplayHandle};

    if handles.backend != NativeWindowBackendV1::Win32 {
        return Err(EngineError::other(format!(
            "unsupported native window backend: {:?}",
            handles.backend
        )));
    }

    let hwnd = NonZeroIsize::new(handles.window as isize)
        .ok_or_else(|| EngineError::other("platform runtime returned null HWND"))?;

    let mut window = Win32WindowHandle::new(hwnd);
    window.hinstance = NonZeroIsize::new(handles.display as isize);

    let display = RawDisplayHandle::Windows(WindowsDisplayHandle::new());
    let window = RawWindowHandle::Win32(window);
    Ok((display, window))
}

#[cfg(not(target_os = "windows"))]
fn native_to_raw_handles(
    _handles: NativeWindowHandlesV1,
) -> EngineResult<(RawDisplayHandle, RawWindowHandle)> {
    Err(EngineError::other(
        "platform runtime native handle conversion is only implemented for Windows",
    ))
}

pub fn detect_platform_runtime_path(modules_dir: &Path) -> EngineResult<PathBuf> {
    type PlatformRuntimeEntryFn = unsafe extern "C" fn(
        abi_stable::std_types::RString,
        newengine_platform_api::PlatformHostApiV1,
        newengine_platform_api::PlatformAppConfigV1,
    ) -> abi_stable::std_types::RResult<(), abi_stable::std_types::RString>;

    #[inline]
    fn is_runtime_candidate(path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            return false;
        };

        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".dll") || lower.ends_with(".so") || lower.ends_with(".dylib")) {
            return false;
        }

        let Ok(lib) = (unsafe { libloading::Library::new(path) }) else {
            return false;
        };

        unsafe { lib.get::<PlatformRuntimeEntryFn>(PLATFORM_RUNTIME_SYMBOL) }.is_ok()
    }

    #[inline]
    fn collect_candidates(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if is_runtime_candidate(&path) {
                out.push(path);
            }
        }
    }

    if let Some(explicit) = std::env::var_os("NEWENGINE_PLATFORM_RUNTIME") {
        let explicit = PathBuf::from(explicit);
        if explicit.is_file() {
            return Ok(explicit);
        }
        return Err(EngineError::other(format!(
            "NEWENGINE_PLATFORM_RUNTIME points to missing file '{}'",
            explicit.display()
        )));
    }

    let exe_dir = std::env::current_exe()
        .map_err(|e| EngineError::other(format!("current_exe failed: {e}")))?
        .parent()
        .ok_or_else(|| EngineError::other("current_exe has no parent"))?
        .to_path_buf();

    let modules_path = if modules_dir.as_os_str().is_empty() || modules_dir == Path::new(".") {
        exe_dir.clone()
    } else if modules_dir.is_absolute() {
        modules_dir.to_path_buf()
    } else {
        exe_dir.join(modules_dir)
    };

    let mut search_dirs: Vec<PathBuf> = vec![
        exe_dir.join("platforms"),
        modules_path.join("platforms"),
        modules_path.clone(),
    ];
    if modules_path != exe_dir {
        search_dirs.push(exe_dir.clone());
    }

    let mut dedup = std::collections::HashSet::new();
    search_dirs.retain(|p| dedup.insert(p.clone()));

    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in &search_dirs {
        collect_candidates(dir, &mut candidates);
    }

    candidates.sort();
    candidates.dedup();

    candidates.into_iter().next().ok_or_else(|| {
        EngineError::other(format!(
            "platform runtime DLL not found; searched [{}] and expected exported symbol 'newengine_platform_runtime_run_v1'",
            search_dirs
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })
}