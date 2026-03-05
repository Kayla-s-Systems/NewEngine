#![forbid(unsafe_op_in_unsafe_fn)]

use crate::system_info::SystemInfo;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

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
    let run_id = crate::run_id::run_id();

    let now = std::time::SystemTime::now();
    let unix_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let mut text = String::new();
    use std::fmt::Write as _;

    let _ = writeln!(text, "{} Crash Report", cfg.product_name);
    let _ = writeln!(text, "Title: {title}");
    let _ = writeln!(text, "App: {} ({})", cfg.app_name, cfg.app_version);
    if let Some(id) = run_id {
        let _ = writeln!(text, "RunId: {id}");
    }
    let _ = writeln!(text, "PID: {}", sys.pid);
    let _ = writeln!(text, "Thread: {thread_name}");
    let _ = writeln!(text, "TimestampUnixMs: {unix_ms}");
    let _ = writeln!(text, "OS: {} {} ({})", sys.os, sys.arch, sys.family);
    if let Some(n) = sys.logical_cpus {
        let _ = writeln!(text, "LogicalCPUs: {n}");
    }
    if let Some(exe) = &sys.exe {
        let _ = writeln!(text, "Exe: {}", exe.display());
    }
    if let Some(cwd) = &sys.cwd {
        let _ = writeln!(text, "Cwd: {}", cwd.display());
    }
    let _ = writeln!(text);
    let _ = writeln!(text, "Details:");
    let _ = writeln!(text, "{details}");

    if let Some(bt) = backtrace {
        let _ = writeln!(text);
        let _ = writeln!(text, "Backtrace:");
        let _ = writeln!(text, "{bt:?}");
    }

    let out_dir = resolve_crash_dir(&cfg, sys.exe.as_deref());
    let out_path = write_report_file(&out_dir, sys.pid, unix_ms, run_id, &text).ok()?;

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

    if let Some(exe) = exe {
        if let Some(dir) = exe.parent() {
            return dir.join(&cfg.crash_dir_name);
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join(&cfg.crash_dir_name)
}

fn write_report_file(
    dir: &Path,
    pid: u32,
    unix_ms: u64,
    run_id: Option<&str>,
    text: &str,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;

    let mut n = 0u32;
    loop {
        let suffix = if n == 0 { String::new() } else { format!("_{n}") };

        let file = if let Some(id) = run_id {
            format!("crash_{unix_ms}_pid{pid}_run{id}{suffix}.txt")
        } else {
            format!("crash_{unix_ms}_pid{pid}{suffix}.txt")
        };
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
            format!("crash reporter not found: {}", exe.display()),
        ));
    }

    let mut cmd = Command::new(&exe);
    cmd.env("NEWENGINE_CRASH_REPORTER_CHILD", "1");
    cmd.arg("--report").arg(report_path);
    cmd.arg("--product").arg(&cfg.product_name);
    cmd.arg("--app").arg(&cfg.app_name);
    cmd.arg("--version").arg(&cfg.app_version);
    match cmd.spawn() {
        Ok(_) => {}
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!(
                    "[newengine] failed to launch crash reporter (not found): '{}'.",
                    exe.display()
                );
            }
            return Err(e);
        }
    }
    Ok(())
}

fn resolve_reporter_path(cfg: &CrashReporterConfig) -> PathBuf {
    let base = cfg.reporter_exe_name.as_str();
    let file = if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_owned()
    };

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join(&file);
            if cand.is_file() {
                return cand;
            }
        }
    }

    PathBuf::from(file)
}