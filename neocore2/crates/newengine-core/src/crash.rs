#![forbid(unsafe_op_in_unsafe_fn)]

use crate::path_fmt::{canonicalize_if_exists, display_clean};
use crate::system_info::SystemInfo;
use newengine_math::collections_prelude::NeVecDeque as VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

/// Crash reporter configuration.
///
/// This is intentionally process-wide. Install once at application startup.
#[derive(Debug, Clone)]
pub struct CrashReporterConfig {
    pub product_name: String,
    pub app_name: String,
    pub app_version: String,

    /// Base directory name to store crash reports under the executable directory.
    ///
    /// Environment override: `NEWENGINE_CRASH_DIR`.
    pub crash_dir_name: String,

    /// Crash reporter executable base name.
    ///
    /// Environment override: `NEWENGINE_CRASH_REPORTER_PATH`.
    pub reporter_exe_name: String,

    /// If true, crash reporter process is spawned when a report is created.
    pub spawn_reporter: bool,
}

impl Default for CrashReporterConfig {
    fn default() -> Self {
        Self {
            product_name: "NewEngine".to_owned(),
            app_name: "app".to_owned(),
            app_version: "0.0.0".to_owned(),
            crash_dir_name: "crash-reports".to_owned(),
            reporter_exe_name: "newengine-crash-reporter".to_owned(),
            spawn_reporter: true,
        }
    }
}

static CFG: OnceLock<CrashReporterConfig> = OnceLock::new();
static PANIC_FIRED: AtomicBool = AtomicBool::new(false);
#[cfg(all(windows, feature = "native-crash-handlers"))]
static WINDOWS_EXCEPTION_FIRED: AtomicBool = AtomicBool::new(false);
static BREADCRUMBS: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
const MAX_BREADCRUMBS: usize = 256;

#[inline]
fn breadcrumbs() -> &'static Mutex<VecDeque<String>> {
    BREADCRUMBS.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_BREADCRUMBS)))
}

/// Records a short diagnostic breadcrumb that will be appended to fatal reports.
///
/// Keep messages compact and stage-oriented. This is intended for last-known-good
/// lifecycle tracking around crashes, panics, and access violations.
#[inline]
pub fn record_breadcrumb(message: impl AsRef<str>) {
    let msg = message.as_ref().trim();
    if msg.is_empty() {
        return;
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let line = format!("[{now}] {msg}");
    let mut guard = match breadcrumbs().lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    if guard.len() >= MAX_BREADCRUMBS {
        let _ = guard.pop_front();
    }
    guard.push_back(line);
}

#[inline]
pub fn clear_breadcrumbs() {
    let mut guard = match breadcrumbs().lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    guard.clear();
}

fn snapshot_breadcrumbs() -> Vec<String> {
    let guard = match breadcrumbs().lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    guard.iter().cloned().collect()
}

/// Installs a process-wide panic hook that:
/// - writes a crash report to disk
/// - spawns `newengine-crash-reporter` (best-effort)
/// - aborts the process
///
/// This should be called as early as possible in `main()`.
pub fn install_panic_hook(cfg: CrashReporterConfig) {
    if std::env::var_os("NEWENGINE_CRASH_REPORTER_CHILD").is_some() {
        return;
    }

    let _ = CFG.set(cfg);
    clear_breadcrumbs();
    record_breadcrumb("crash: panic hook installed");
    install_native_crash_handlers();

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if PANIC_FIRED.swap(true, Ordering::AcqRel) {
            prev(info);
            std::process::abort();
        }

        let bt = std::backtrace::Backtrace::force_capture();
        let msg = panic_message(info);
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_owned());

        record_breadcrumb(format!("panic: msg='{msg}' location='{loc}'"));

        let title = "Unhandled panic";
        let details = format!("{msg}\n\nLocation:\n{loc}\n\nBacktrace:\n{bt:?}\n");

        let _ = report_fatal_impl(title, &details, Some(&bt));

        prev(info);
        std::process::abort();
    }));
}

/// Writes a crash report and spawns the crash reporter window.
///
/// Use this for unrecoverable `EngineError` paths where you want a UE-like crash dialog.
pub fn report_fatal(title: &str, details: &str) -> Option<PathBuf> {
    record_breadcrumb(format!("fatal: title='{title}'"));
    let bt = std::backtrace::Backtrace::force_capture();
    report_fatal_impl(title, details, Some(&bt))
}

fn report_fatal_impl(
    title: &str,
    details: &str,
    backtrace: Option<&std::backtrace::Backtrace>,
) -> Option<PathBuf> {
    let cfg = CFG.get().cloned().unwrap_or_default();
    let sys = SystemInfo::collect();
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("<unnamed>");

    let now = std::time::SystemTime::now();
    let unix_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let rid = crate::run_id::run_id().unwrap_or("<unknown>");
    let rtag = crate::run_id::run_tag().unwrap_or("<unknown>");
    let breadcrumbs = snapshot_breadcrumbs();
    let services = newengine_plugin_host::list_services();
    let runtimes = newengine_plugin_host::list_external_runtime_plugins();

    let mut text = String::new();
    use std::fmt::Write as _;

    let _ = writeln!(text, "{} Crash Report", cfg.product_name);
    let _ = writeln!(text, "Title: {title}");
    let _ = writeln!(text, "App: {} ({})", cfg.app_name, cfg.app_version);
    let _ = writeln!(text, "PID: {}", sys.pid);
    let _ = writeln!(text, "Thread: {thread_name}");
    let _ = writeln!(text, "TimestampUnixMs: {unix_ms}");
    let _ = writeln!(text, "RunTag: {rtag}");
    let _ = writeln!(text, "RunId: {rid}");
    let _ = writeln!(text, "OS: {} {} ({})", sys.os, sys.arch, sys.family);
    if let Some(n) = sys.logical_cpus {
        let _ = writeln!(text, "LogicalCPUs: {n}");
    }
    if let Some(exe) = &sys.exe {
        let _ = writeln!(text, "Exe: {}", display_clean(&canonicalize_if_exists(exe)));
    }
    if let Some(cwd) = &sys.cwd {
        let _ = writeln!(text, "Cwd: {}", display_clean(&canonicalize_if_exists(cwd)));
    }
    if let Some(log_file) = std::env::var_os("NEWENGINE_LOG_FILE") {
        let _ = writeln!(text, "LogFile: {}", display_clean(&canonicalize_if_exists(&PathBuf::from(log_file))));
    }

    let _ = writeln!(text);
    let _ = writeln!(text, "Details:");
    let _ = writeln!(text, "{details}");

    if !breadcrumbs.is_empty() {
        let _ = writeln!(text);
        let _ = writeln!(text, "Breadcrumbs:");
        for line in breadcrumbs {
            let _ = writeln!(text, "- {line}");
        }
    }

    let _ = writeln!(text);
    let _ = writeln!(text, "PluginHost:");
    let _ = writeln!(text, "- services_count: {}", services.len());
    for service in services {
        let _ = writeln!(text, "  - service: {service}");
    }
    let _ = writeln!(text, "- external_runtimes_count: {}", runtimes.len());
    for runtime in runtimes {
        let _ = writeln!(
            text,
            "  - runtime: id='{}' ver='{}' state='{}' path='{}' caps={}",
            runtime.id,
            runtime.version,
            runtime.state,
            display_clean(&canonicalize_if_exists(&runtime.path)),
            runtime.capabilities.len()
        );
    }

    if let Some(bt) = backtrace {
        let _ = writeln!(text);
        let _ = writeln!(text, "Backtrace:");
        let _ = writeln!(text, "{bt:?}");
    }

    let out_dir = resolve_crash_dir(&cfg, sys.exe.as_deref());
    let out_path = write_report_file(&out_dir, sys.pid, unix_ms, rid, rtag, &text).ok()?;

    eprintln!("[newengine] crash report written: {}", display_clean(&canonicalize_if_exists(&out_path)));

    if cfg.spawn_reporter {
        let _ = spawn_reporter(&cfg, &out_path);
    }

    Some(out_path)
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

fn resolve_crash_dir(cfg: &CrashReporterConfig, exe: Option<&Path>) -> PathBuf {
    if let Some(p) = std::env::var_os("NEWENGINE_CRASH_DIR") {
        return PathBuf::from(p);
    }

    if let Some(cache) = std::env::var_os(crate::cache_files::CACHE_FILES_ENV)
        .or_else(|| std::env::var_os(crate::cache_files::CACHE_FILES_ALIAS_ENV))
    {
        return PathBuf::from(cache).join(&cfg.crash_dir_name);
    }

    if let Some(exe) = exe {
        if let Some(dir) = exe.parent() {
            return dir.join("cache").join(&cfg.crash_dir_name);
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("cache")
        .join(&cfg.crash_dir_name)
}

fn write_report_file(
    dir: &Path,
    pid: u32,
    unix_ms: u64,
    _run_id: &str,
    run_tag: &str,
    text: &str,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;

    let mut n = 0u32;
    loop {
        let suffix = if n == 0 { String::new() } else { format!("_{n}") };

        let file = format!("crash_{unix_ms}_pid{pid}_run{run_tag}{suffix}.txt");
        let path = dir.join(file);

        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            Ok(mut f) => {
                use std::io::Write as _;
                f.write_all(text.as_bytes())?;
                let _ = f.flush();
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                n = n.saturating_add(1);
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

fn spawn_reporter(cfg: &CrashReporterConfig, report_path: &Path) -> std::io::Result<()> {
    use std::process::Command;

    let exe = if let Some(p) = std::env::var_os("NEWENGINE_CRASH_REPORTER_PATH") {
        PathBuf::from(p)
    } else {
        resolve_reporter_path(cfg)
    };

    if !exe.is_file() {
        eprintln!(
            "[newengine] crash reporter not found: '{}'. Expected '{}' next to the app or set NEWENGINE_CRASH_REPORTER_PATH.",
            exe.display(),
            cfg.reporter_exe_name
        );
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "crash reporter not found",
        ));
    }

    // no-hidden-thread-scan: crash reporter is an explicit post-fault diagnostic child process, not runtime work.
    let mut cmd = Command::new(exe);
    cmd.env("NEWENGINE_CRASH_REPORTER_CHILD", "1");
    cmd.arg(report_path);

    let _ = cmd.spawn()?;
    Ok(())
}

fn resolve_reporter_path(cfg: &CrashReporterConfig) -> PathBuf {
    let base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()));

    let name = if cfg!(windows) {
        format!("{}.exe", cfg.reporter_exe_name)
    } else {
        cfg.reporter_exe_name.clone()
    };

    base.join(name)
}

#[cfg(all(windows, feature = "native-crash-handlers"))]
fn install_native_crash_handlers() {
    use windows::Win32::System::Diagnostics::Debug::SetUnhandledExceptionFilter;

    unsafe {
        let _ = SetUnhandledExceptionFilter(Some(unhandled_exception_filter));
    }

    record_breadcrumb("crash: windows unhandled exception filter installed");
}

#[cfg(not(all(windows, feature = "native-crash-handlers")))]
fn install_native_crash_handlers() {}

#[cfg(all(windows, feature = "native-crash-handlers"))]
unsafe extern "system" fn unhandled_exception_filter(
    info: *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS,
) -> i32 {
    use windows::Win32::Foundation::{
        EXCEPTION_ACCESS_VIOLATION,
        EXCEPTION_ARRAY_BOUNDS_EXCEEDED,
        EXCEPTION_BREAKPOINT,
        EXCEPTION_DATATYPE_MISALIGNMENT,
        EXCEPTION_FLT_DIVIDE_BY_ZERO,
        EXCEPTION_ILLEGAL_INSTRUCTION,
        EXCEPTION_INT_DIVIDE_BY_ZERO,
        EXCEPTION_IN_PAGE_ERROR,
        EXCEPTION_STACK_OVERFLOW,
        NTSTATUS,
    };
    use windows::Win32::System::Diagnostics::Debug::EXCEPTION_EXECUTE_HANDLER;

    if WINDOWS_EXCEPTION_FIRED.swap(true, Ordering::AcqRel) {
        return EXCEPTION_EXECUTE_HANDLER;
    }

    let mut code = NTSTATUS(0);
    let mut address = 0usize;
    let mut access_kind = None::<usize>;
    let mut access_addr = None::<usize>;

    if !info.is_null() {
        let rec = unsafe { (*info).ExceptionRecord };
        if !rec.is_null() {
            code = unsafe { (*rec).ExceptionCode };
            address = unsafe { (*rec).ExceptionAddress as usize };

            if code == EXCEPTION_ACCESS_VIOLATION {
                let count = unsafe { (*rec).NumberParameters as usize };
                if count >= 2 {
                    access_kind = Some(unsafe { (*rec).ExceptionInformation[0] });
                    access_addr = Some(unsafe { (*rec).ExceptionInformation[1] });
                }
            }
        }
    }

    let kind = match code {
        c if c == EXCEPTION_ACCESS_VIOLATION => "EXCEPTION_ACCESS_VIOLATION",
        c if c == EXCEPTION_IN_PAGE_ERROR => "EXCEPTION_IN_PAGE_ERROR",
        c if c == EXCEPTION_STACK_OVERFLOW => "EXCEPTION_STACK_OVERFLOW",
        c if c == EXCEPTION_ILLEGAL_INSTRUCTION => "EXCEPTION_ILLEGAL_INSTRUCTION",
        c if c == EXCEPTION_INT_DIVIDE_BY_ZERO => "EXCEPTION_INT_DIVIDE_BY_ZERO",
        c if c == EXCEPTION_FLT_DIVIDE_BY_ZERO => "EXCEPTION_FLT_DIVIDE_BY_ZERO",
        c if c == EXCEPTION_ARRAY_BOUNDS_EXCEEDED => "EXCEPTION_ARRAY_BOUNDS_EXCEEDED",
        c if c == EXCEPTION_DATATYPE_MISALIGNMENT => "EXCEPTION_DATATYPE_MISALIGNMENT",
        c if c == EXCEPTION_BREAKPOINT => "EXCEPTION_BREAKPOINT",
        _ => "UNKNOWN_EXCEPTION",
    };

    let extra = match (access_kind, access_addr) {
        (Some(k), Some(a)) => format!("\nAccessKind: {k}\nAccessAddress: 0x{a:X}"),
        _ => String::new(),
    };

    record_breadcrumb(format!(
        "seh: code=0x{:08X} kind={} address=0x{:X}",
        code.0 as u32,
        kind,
        address
    ));

    let details = format!(
        "Unhandled Windows exception\n\nExceptionCode: 0x{:08X}\nExceptionKind: {}\nExceptionAddress: 0x{:X}{}",
        code.0 as u32,
        kind,
        address,
        extra
    );

    let _ = report_fatal_impl("Unhandled Windows Exception", &details, None);

    EXCEPTION_EXECUTE_HANDLER
}