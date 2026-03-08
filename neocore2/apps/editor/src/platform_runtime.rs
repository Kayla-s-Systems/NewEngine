#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashSet;
use std::num::NonZeroIsize;
use std::path::{Path, PathBuf};

use abi_stable::std_types::{RResult, RString, RVec};
use libloading::Library;
use newengine_core::events::EventSub;
use newengine_core::host_events::{
    CursorGrabMode, CursorState, HostEvent, WindowHandles, WindowHostEvent, WindowInitSize,
};
use newengine_core::{Engine, EngineError, EngineResult, StartupConfig};
use newengine_platform_api::{
    NativeWindowBackendV1, NativeWindowHandlesV1, PlatformAppConfigV1, PlatformCursorGrabModeV1,
    PlatformCursorPollV1, PlatformCursorStateV1, PlatformHostApiV1, PlatformRuntimeRunFnV1,
    PlatformStepResultV1, PlatformSurfaceMetricsV1, PlatformWindowPlacementKindV1,
    PlatformWindowPlacementV1, PlatformWindowReadyV1,
};
use newengine_plugin_api::{
    ConfigBlobV1, ConfigDiagLevelV1, ConfigPatchSourceV1, ConfigPatchV1, HostApiV1,
    PluginRootV1Ref, PluginSignatureV1,
};
use newengine_ui::{
    create_provider, UiBuildFn, UiFrameDesc, UiProvider, UiProviderKind, UiProviderOptions,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use serde_json::Value;

use crate::platform_input::poll_input_frame;

const PLATFORM_RUNTIME_SYMBOL: &[u8] = b"newengine_platform_runtime_run_v1\0";
const PLUGIN_ROOT_SYMBOL: &[u8] = b"export_plugin_root\0";
const PLUGIN_SIGNATURE_SYMBOL: &[u8] = b"newengine_plugin_signature_v1\0";
const PLATFORM_PLUGIN_ID: &str = "newengine.platform.winit";
const CT_JSON_MERGE_PATCH: &str = "application/merge-patch+json";

pub struct ResolvedPlatformRuntimeConfig {
    pub plugin_id: String,
    pub config: PlatformAppConfigV1,
    pub icon_path: Option<String>,
}

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

        let run: libloading::Symbol<PlatformRuntimeRunFnV1> = unsafe { lib.get(PLATFORM_RUNTIME_SYMBOL) }
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

        let plugin_host = newengine_plugin_host::default_host_api();
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

#[inline]
pub fn legacy_platform_config_from_startup(startup: &StartupConfig) -> PlatformAppConfigV1 {
    let placement = match startup.window_placement {
        newengine_core::startup::WindowPlacement::Default => PlatformWindowPlacementV1 {
            kind: PlatformWindowPlacementKindV1::OsDefault,
            x: 0,
            y: 0,
        },
        newengine_core::startup::WindowPlacement::Centered { offset } => PlatformWindowPlacementV1 {
            kind: PlatformWindowPlacementKindV1::Centered,
            x: offset.0,
            y: offset.1,
        },
    };

    PlatformAppConfigV1 {
        title: startup.window_title.clone().into(),
        width: startup.window_size.0,
        height: startup.window_size.1,
        placement,
        icon: abi_stable::std_types::ROption::RNone,
    }
}

#[inline]
fn config_patch_from_json_merge_patch(name: &str, priority: i32, value: &Value) -> ConfigPatchV1 {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    ConfigPatchV1 {
        source: ConfigPatchSourceV1::HostRule,
        content_type: RString::from(CT_JSON_MERGE_PATCH),
        bytes: RVec::from(bytes),
        priority,
        name: RString::from(name),
    }
}

#[inline]
fn is_non_empty_object(value: &Value) -> bool {
    matches!(value, Value::Object(map) if !map.is_empty())
}

fn platform_config_from_effective_blob(blob: &ConfigBlobV1) -> Result<PlatformAppConfigV1, String> {
    if blob.content_type.as_str() != "application/json" {
        return Err(format!(
            "unsupported platform config content_type '{}'",
            blob.content_type
        ));
    }

    let value: Value = serde_json::from_slice(blob.bytes.as_slice())
        .map_err(|e| format!("platform config parse failed: {e}"))?;

    let obj = value
        .as_object()
        .ok_or_else(|| "platform config must be a JSON object".to_owned())?;

    let title = obj
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("NewEngine")
        .to_owned();

    let width = obj
        .get("width")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .unwrap_or(1600)
        .clamp(64, 16384);

    let height = obj
        .get("height")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .unwrap_or(900)
        .clamp(64, 16384);

    let placement_obj = obj.get("placement").and_then(Value::as_object);
    let placement_mode = placement_obj
        .and_then(|it| it.get("mode"))
        .and_then(Value::as_str)
        .unwrap_or("os_default");

    let placement = match placement_mode {
        "os_default" | "default" => PlatformWindowPlacementV1 {
            kind: PlatformWindowPlacementKindV1::OsDefault,
            x: 0,
            y: 0,
        },
        "centered" | "center" | "centre" => PlatformWindowPlacementV1 {
            kind: PlatformWindowPlacementKindV1::Centered,
            x: placement_obj
                .and_then(|it| it.get("x"))
                .and_then(Value::as_i64)
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0),
            y: placement_obj
                .and_then(|it| it.get("y"))
                .and_then(Value::as_i64)
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0),
        },
        "absolute" => PlatformWindowPlacementV1 {
            kind: PlatformWindowPlacementKindV1::Absolute,
            x: placement_obj
                .and_then(|it| it.get("x"))
                .and_then(Value::as_i64)
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0),
            y: placement_obj
                .and_then(|it| it.get("y"))
                .and_then(Value::as_i64)
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0),
        },
        other => {
            return Err(format!("unsupported placement.mode '{other}'"));
        }
    };

    Ok(PlatformAppConfigV1 {
        title: title.into(),
        width,
        height,
        placement,
        icon: abi_stable::std_types::ROption::RNone,
    })
}

fn log_platform_config_diags(plugin_id: &str, diags: &[newengine_plugin_api::ConfigDiagV1]) {
    for diag in diags {
        match diag.level {
            ConfigDiagLevelV1::Info => log::info!(
                "platform runtime: config info id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
            ConfigDiagLevelV1::Warn => log::warn!(
                "platform runtime: config warn id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
            ConfigDiagLevelV1::Error => log::error!(
                "platform runtime: config error id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
        }
    }
}

fn try_read_runtime_identity(path: &Path) -> Option<(String, String)> {
    let lib = unsafe { Library::new(path) }.ok()?;
    let has_runtime = unsafe { lib.get::<PlatformRuntimeRunFnV1>(PLATFORM_RUNTIME_SYMBOL) }.is_ok();
    if !has_runtime {
        return None;
    }

    if let Ok(sym) = unsafe { lib.get::<unsafe extern "C" fn() -> PluginSignatureV1>(PLUGIN_SIGNATURE_SYMBOL) } {
        let signature = unsafe { sym() };
        let id = signature.id.to_string();
        let version = signature.version.to_string();
        if !id.trim().is_empty() {
            return Some((id, version));
        }
    }

    let root_sym = unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL) }.ok()?;
    let root = unsafe { root_sym() };

    if let Some(create_v3) = root.create_v3() {
        let module = create_v3();
        let descriptor = module.descriptor_v3();
        return Some((descriptor.id.to_string(), descriptor.version.to_string()));
    }

    if let Some(create_v2) = root.create_v2() {
        let module = create_v2();
        let descriptor = module.descriptor();
        return Some((descriptor.id.to_string(), descriptor.version.to_string()));
    }

    let module = root.create()();
    let info = module.info();
    Some((info.id.to_string(), info.version.to_string()))
}

fn extract_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|it| !it.is_empty())
        .map(str::to_owned)
}

fn strip_host_only_platform_keys(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(obj) = value.as_object_mut() {
        obj.remove("icon");
    }
    value
}

pub fn resolve_platform_runtime_config(
    startup: &StartupConfig,
    runtime_path: &Path,
) -> EngineResult<ResolvedPlatformRuntimeConfig> {
    let legacy = legacy_platform_config_from_startup(startup);
    let lib = unsafe { Library::new(runtime_path) }
        .map_err(|e| EngineError::other(format!("platform runtime metadata load failed: {e}")))?;

    let root_sym = match unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL) } {
        Ok(sym) => sym,
        Err(_) => {
            log::info!("platform runtime: plugin metadata not exported; using legacy startup window config");
            return Ok(ResolvedPlatformRuntimeConfig {
                plugin_id: PLATFORM_PLUGIN_ID.to_owned(),
                config: legacy,
                icon_path: startup.window_icon_path.clone(),
            });
        }
    };

    let root = unsafe { root_sym() };
    let Some(create_v3) = root.create_v3() else {
        log::info!("platform runtime: plugin metadata ABI V3 not available; using legacy startup window config");
        return Ok(ResolvedPlatformRuntimeConfig {
            plugin_id: PLATFORM_PLUGIN_ID.to_owned(),
            config: legacy,
            icon_path: startup.window_icon_path.clone(),
        });
    };

    let module = create_v3();
    let descriptor = module.descriptor_v3();
    let plugin_id = descriptor.id.to_string();

    let defaults = module
        .config_defaults_v1()
        .into_result()
        .map_err(|e| EngineError::other(format!("platform config defaults failed: {e}")))?;

    let overrides = newengine_plugin_host::get_plugin_overrides_with_env(&plugin_id);
    let icon_path = extract_string_field(&overrides, "icon");
    let plugin_patch = strip_host_only_platform_keys(&overrides);

    let mut patches = RVec::<ConfigPatchV1>::new();
    if is_non_empty_object(&plugin_patch) {
        patches.push(config_patch_from_json_merge_patch("config+env", 0, &plugin_patch));
    }

    let applied = module
        .config_apply_patches_v1(&defaults, patches)
        .into_result()
        .map_err(|e| EngineError::other(format!("platform config apply failed: {e}")))?;

    log_platform_config_diags(&plugin_id, applied.diags.as_slice());

    let config = platform_config_from_effective_blob(&applied.effective)
        .map_err(|e| EngineError::other(format!("platform config decode failed: {e}")))?;

    log::info!(
        "platform runtime: effective config id='{}' title='{}' size={}x{} placement={:?} icon={} ",
        plugin_id,
        config.title,
        config.width,
        config.height,
        config.placement.kind,
        icon_path.as_deref().unwrap_or("<none>")
    );

    Ok(ResolvedPlatformRuntimeConfig {
        plugin_id,
        config,
        icon_path,
    })
}

pub fn detect_platform_runtime_path(modules_dir: &Path) -> EngineResult<PathBuf> {
    type PlatformRuntimeEntryFn = unsafe extern "C" fn(
        HostApiV1,
        PlatformHostApiV1,
        PlatformAppConfigV1,
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

    let mut dedup = HashSet::new();
    search_dirs.retain(|p| dedup.insert(p.clone()));

    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in &search_dirs {
        collect_candidates(dir, &mut candidates);
    }

    candidates.sort();
    candidates.dedup();

    if let Some(path) = candidates
        .iter()
        .find(|path| matches!(try_read_runtime_identity(path), Some((ref id, _)) if id == PLATFORM_PLUGIN_ID))
        .cloned()
    {
        return Ok(path);
    }

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
