#![forbid(unsafe_op_in_unsafe_fn)]

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use crate::startup::config::StartupLoggingConfig;
use crate::startup::system_probe::SystemProbe;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::Layer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static LOG_INIT: OnceLock<()> = OnceLock::new();

/// Keeps background logging worker alive.
///
/// Must be held for the entire lifetime of the process.
#[derive(Debug)]
pub struct StartupLogHandle {
    _guard: WorkerGuard,
}

fn filter_from_cfg(cfg: &StartupLoggingConfig) -> EnvFilter {
    if let Some(spec) = cfg
        .filter
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return EnvFilter::new(spec.to_owned());
    }

    let lvl = cfg.level.trim();
    if !lvl.is_empty() {
        return EnvFilter::new(lvl.to_owned());
    }

    EnvFilter::new("info")
}

fn resolved_filter_spec(cfg: &StartupLoggingConfig) -> String {
    if let Some(spec) = cfg
        .filter
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return spec.to_owned();
    }

    let lvl = cfg.level.trim();
    if !lvl.is_empty() {
        return lvl.to_owned();
    }

    "info".to_owned()
}

fn want_timestamp(ts: Option<&str>) -> bool {
    match ts.map(str::trim) {
        None => true,
        Some("") => true,
        Some("none") => false,
        Some(_) => true,
    }
}

#[derive(Clone, Copy, Debug)]
struct StartupTime {
    enabled: bool,
}

impl fmt::time::FormatTime for StartupTime {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        if !self.enabled {
            return Ok(());
        }
        fmt::time::SystemTime.format_time(w)
    }
}

fn mk_fmt_layer<W>(
    cfg: &StartupLoggingConfig,
    timer: StartupTime,
    ansi: bool,
    writer: W,
) -> fmt::Layer<
    tracing_subscriber::Registry,
    fmt::format::DefaultFields,
    fmt::format::Format<fmt::format::Full, StartupTime>,
    W,
>
where
    W: for<'a> fmt::MakeWriter<'a> + Send + Sync + 'static,
{
    // If you want a standard behavior, replace with:
    // let want_target = cfg.include_target;
    let want_target = cfg.include_target && cfg.include_module_path;

    fmt::layer()
        .with_ansi(ansi)
        .with_target(want_target)
        .with_file(cfg.include_file)
        .with_line_number(cfg.include_line_number)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_timer(timer)
        .with_writer(writer)
}

fn resolve_console_writer(cfg: &StartupLoggingConfig) -> fmt::writer::BoxMakeWriter {
    let to_stderr = cfg
        .console_target
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("stderr"))
        .unwrap_or(true);

    if to_stderr {
        fmt::writer::BoxMakeWriter::new(|| std::io::stderr())
    } else {
        fmt::writer::BoxMakeWriter::new(|| std::io::stdout())
    }
}

fn sanitize_path_for_banner(p: &str) -> String {
    p.replace('\n', " ").replace('\r', " ")
}

fn emit_startup_banner_v2(
    cfg: &StartupLoggingConfig,
    filter_spec: &str,
    log_mode: &str,
    log_file: Option<&str>,
) {
    use std::env;

    let exe = env::current_exe().ok();
    let cwd = env::current_dir().ok();

    let engine_name = option_env!("CARGO_PKG_NAME").unwrap_or("newengine");
    let engine_ver = option_env!("CARGO_PKG_VERSION").unwrap_or("0.0.0");

    let git_sha = option_env!("VERGEN_GIT_SHA")
        .or(option_env!("GIT_SHA"))
        .or(option_env!("SOURCE_GIT_SHA"));

    let target = option_env!("VERGEN_CARGO_TARGET_TRIPLE")
        .or(option_env!("TARGET"))
        .unwrap_or(std::env::consts::ARCH);

    let build_ts = option_env!("VERGEN_BUILD_TIMESTAMP").or(option_env!("BUILD_TIMESTAMP"));

    let sys = SystemProbe::probe();

    let exe_s = exe
        .as_ref()
        .map(|p| sanitize_path_for_banner(&p.display().to_string()))
        .unwrap_or_else(|| "<unknown>".to_owned());

    let cwd_s = cwd
        .as_ref()
        .map(|p| sanitize_path_for_banner(&p.display().to_string()))
        .unwrap_or_else(|| "<unknown>".to_owned());

    let log_file_s = log_file
        .map(sanitize_path_for_banner)
        .unwrap_or_else(|| "<none>".to_owned());

    let git_s = git_sha.map(|s| format!(" ({s})")).unwrap_or_default();

    // Pretty (console-focused). Keep it readable.
    tracing::info!(
        target: "startup.banner.pretty",
        "=== {engine_name} STARTUP ===\n\
         version      : {engine_ver}{git}\n\
         target       : {target} ({os})\n\
         pid          : {pid}\n\
         exe          : {exe}\n\
         cwd          : {cwd}\n\
         \n\
         cpu          : {cpu}\n\
         cpu.cores    : {cores}\n\
         ram.total_mb : {ram}\n\
         gpu          : {gpu}\n\
         vram.mb      : {vram}\n\
         directx      : {dx}\n\
         \n\
         log.mode     : {log_mode}\n\
         log.filter   : {filter}\n\
         log.file     : {log_file}\n\
         log.colors   : {colors}\n\
         log.timestamp: {timestamp}\n\
         log.target   : {tgt}\n\
         build.ts     : {src}\n\
         === STARTUP END ===",
        git = git_s,
        os = std::env::consts::OS,
        pid = std::process::id(),
        exe = exe_s,
        cwd = cwd_s,
        cpu = sys.cpu.as_deref().unwrap_or("unknown"),
        cores = sys.cpu_cores_logical.map(|v| v.to_string()).unwrap_or_else(|| "unknown".to_owned()),
        ram = sys.ram_total_mb.map(|v| v.to_string()).unwrap_or_else(|| "unknown".to_owned()),
        gpu = sys.gpu.as_deref().unwrap_or("unknown"),
        vram = sys.vram_dedicated_mb.map(|v| v.to_string()).unwrap_or_else(|| "unknown".to_owned()),
        dx = sys.directx.as_deref().unwrap_or("unknown"),
        log_mode = log_mode,
        filter = filter_spec,
        log_file = log_file_s,
        colors = cfg.colors,
        timestamp = cfg.timestamp.as_deref().unwrap_or("millis"),
        tgt = cfg.console_target.as_deref().unwrap_or("stderr"),
        src = build_ts.unwrap_or("unknown"),
    );

    // Compact (file/CI-friendly). One line.
    tracing::info!(
        target: "startup.banner",
        "startup engine={} ver={}{} target={} os={} pid={} exe=\"{}\" cwd=\"{}\" \
         cpu=\"{}\" cores={} ram_mb={} gpu=\"{}\" vram_mb={} dx=\"{}\" \
         log_mode={} log_filter=\"{}\" log_file=\"{}\" build_ts=\"{}\"",
        engine_name,
        engine_ver,
        git_s,
        target,
        std::env::consts::OS,
        std::process::id(),
        exe_s,
        cwd_s,
        sys.cpu.as_deref().unwrap_or("unknown"),
        sys.cpu_cores_logical.unwrap_or(0),
        sys.ram_total_mb.unwrap_or(0),
        sys.gpu.as_deref().unwrap_or("unknown"),
        sys.vram_dedicated_mb.unwrap_or(0),
        sys.directx.as_deref().unwrap_or("unknown"),
        log_mode,
        filter_spec,
        log_file_s,
        build_ts.unwrap_or("unknown"),
    );
}

fn set_subscriber_console_only(
    filter: EnvFilter,
    console_layer: fmt::Layer<
        tracing_subscriber::Registry,
        fmt::format::DefaultFields,
        fmt::format::Format<fmt::format::Full, StartupTime>,
        fmt::writer::BoxMakeWriter,
    >,
) -> bool {
    let subscriber = tracing_subscriber::registry()
        .with(console_layer)
        .with(filter);
    tracing::subscriber::set_global_default(subscriber).is_ok()
}

fn set_subscriber_file_only(
    filter: EnvFilter,
    file_layer: fmt::Layer<
        tracing_subscriber::Registry,
        fmt::format::DefaultFields,
        fmt::format::Format<fmt::format::Full, StartupTime>,
        tracing_appender::non_blocking::NonBlocking,
    >,
) -> bool {
    let subscriber = tracing_subscriber::registry()
        .with(file_layer)
        .with(filter);
    tracing::subscriber::set_global_default(subscriber).is_ok()
}

fn set_subscriber_console_and_file(
    filter: EnvFilter,
    console_layer: fmt::Layer<
        tracing_subscriber::Registry,
        fmt::format::DefaultFields,
        fmt::format::Format<fmt::format::Full, StartupTime>,
        fmt::writer::BoxMakeWriter,
    >,
    file_layer: fmt::Layer<
        tracing_subscriber::Registry,
        fmt::format::DefaultFields,
        fmt::format::Format<fmt::format::Full, StartupTime>,
        tracing_appender::non_blocking::NonBlocking,
    >,
) -> bool {
    // ✅ Combine BOTH fmt layers into one layer that still targets Registry.
    let combined = console_layer.and_then(file_layer);

    // EnvFilter must be last.
    let subscriber = tracing_subscriber::registry()
        .with(combined)
        .with(filter);

    tracing::subscriber::set_global_default(subscriber).is_ok()
}

/// Initializes process-wide logging according to startup config.
///
/// Guarantees:
/// - installs a global subscriber at most once
/// - if file output fails, falls back to console-only and prints a stderr explanation
/// - file output never contains ANSI escape sequences (even in tee mode)
pub fn init_startup_logging(
    cfg: StartupLoggingConfig,
) -> Result<Option<StartupLogHandle>, Box<dyn std::error::Error>> {
    if LOG_INIT.get().is_some() {
        return Ok(None);
    }

    let _ = tracing_log::LogTracer::init();

    let filter = filter_from_cfg(&cfg);
    let filter_spec = resolved_filter_spec(&cfg);

    let timer = StartupTime {
        enabled: want_timestamp(cfg.timestamp.as_deref()),
    };

    let mut file_guard: Option<StartupLogHandle> = None;
    let mut file_writer = None::<tracing_appender::non_blocking::NonBlocking>;
    let mut file_path_for_banner: Option<String> = None;

    if let Some(path) = cfg.file_path.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let p = Path::new(path);

        let file_name = match p.file_name().and_then(|v| v.to_str()).map(str::trim) {
            Some(s) if !s.is_empty() => s,
            _ => {
                eprintln!(
                    "startup: logging: invalid log file name '{}'. Falling back to console-only.",
                    p.display()
                );
                ""
            }
        };

        if !file_name.is_empty() {
            let dir = p.parent().unwrap_or_else(|| Path::new("."));
            if let Err(e) = fs::create_dir_all(dir) {
                eprintln!(
                    "startup: logging: failed to create log directory '{}': {e}. Falling back to console-only.",
                    dir.display()
                );
            } else {
                let appender = tracing_appender::rolling::never(dir, file_name);
                let (nb, guard) = tracing_appender::non_blocking(appender);
                file_path_for_banner = Some(p.display().to_string());
                file_writer = Some(nb);
                file_guard = Some(StartupLogHandle { _guard: guard });
            }
        }
    }

    let has_file = file_writer.is_some();
    let console_enabled = !has_file || cfg.tee;
    let file_enabled = has_file;

    let installed = match (console_enabled, file_enabled) {
        (true, true) => {
            let console_writer = resolve_console_writer(&cfg);
            let console_layer = mk_fmt_layer(&cfg, timer, cfg.colors, console_writer);

            let nb = file_writer.clone().expect("file_writer must exist");
            let file_layer = mk_fmt_layer(&cfg, timer, false, nb);

            set_subscriber_console_and_file(filter, console_layer, file_layer)
        }
        (true, false) => {
            let console_writer = resolve_console_writer(&cfg);
            let console_layer = mk_fmt_layer(&cfg, timer, cfg.colors, console_writer);

            set_subscriber_console_only(filter, console_layer)
        }
        (false, true) => {
            let nb = file_writer.clone().expect("file_writer must exist");
            let file_layer = mk_fmt_layer(&cfg, timer, false, nb);

            set_subscriber_file_only(filter, file_layer)
        }
        (false, false) => {
            let subscriber = tracing_subscriber::registry().with(filter);
            tracing::subscriber::set_global_default(subscriber).is_ok()
        }
    };

    if !installed {
        return Ok(None);
    }

    let _ = LOG_INIT.set(());

    let log_mode = match (console_enabled, file_enabled) {
        (true, true) => "console+file",
        (true, false) => "console",
        (false, true) => "file",
        (false, false) => "none",
    };

    emit_startup_banner_v2(
        &cfg,
        &filter_spec,
        log_mode,
        file_path_for_banner.as_deref(),
    );

    Ok(file_guard)
}