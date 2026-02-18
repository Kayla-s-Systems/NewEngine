#![forbid(unsafe_op_in_unsafe_fn)]

use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::startup::config::StartupLoggingConfig;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

static LOG_INIT: OnceLock<()> = OnceLock::new();

/// Keeps background logging worker alive.
///
/// Must be held for the entire lifetime of the process.
#[derive(Debug)]
pub struct StartupLogHandle {
    _guard: WorkerGuard,
}

fn filter_from_cfg(cfg: &StartupLoggingConfig, legacy_level: Option<&str>) -> EnvFilter {
    if let Some(spec) = cfg.filter.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return EnvFilter::new(spec.to_owned());
    }

    let lvl = cfg.level.trim();
    if !lvl.is_empty() {
        return EnvFilter::new(lvl.to_owned());
    }

    if let Some(legacy) = legacy_level.map(str::trim).filter(|s| !s.is_empty()) {
        return EnvFilter::new(legacy.to_owned());
    }

    EnvFilter::new("info")
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

/// Writer that duplicates writes into two `io::Write` sinks.
struct TeeWriter<A: io::Write, B: io::Write> {
    a: A,
    b: B,
}

impl<A: io::Write, B: io::Write> io::Write for TeeWriter<A, B> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.a.write_all(buf)?;
        self.b.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.a.flush()?;
        self.b.flush()?;
        Ok(())
    }
}

/// MakeWriter impl backed by a dynamically dispatched factory.
struct DynMakeWriter {
    mk: Arc<dyn Fn() -> Box<dyn io::Write + Send> + Send + Sync>,
}

impl<'a> fmt::MakeWriter<'a> for DynMakeWriter {
    type Writer = Box<dyn io::Write + Send>;

    fn make_writer(&'a self) -> Self::Writer {
        (self.mk)()
    }
}

fn mk_fmt_layer(
    cfg: &StartupLoggingConfig,
    timer: StartupTime,
    ansi: bool,
    writer: fmt::writer::BoxMakeWriter,
) -> fmt::Layer<
    tracing_subscriber::Registry,
    fmt::format::DefaultFields,
    fmt::format::Format<fmt::format::Full, StartupTime>,
    fmt::writer::BoxMakeWriter,
> {
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

/// Initializes process-wide logging according to startup config.
pub fn init_startup_logging(
    cfg: StartupLoggingConfig,
    legacy_level: Option<&str>,
) -> Result<Option<StartupLogHandle>, Box<dyn std::error::Error>> {
    if LOG_INIT.get().is_some() {
        return Ok(None);
    }

    let _ = tracing_log::LogTracer::init();

    let filter = filter_from_cfg(&cfg, legacy_level);

    let timer = StartupTime {
        enabled: want_timestamp(cfg.timestamp.as_deref()),
    };

    // Console target.
    let console_to_stderr = cfg
        .console_target
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("stderr"))
        .unwrap_or(true);

    let console_mk: Arc<dyn Fn() -> Box<dyn io::Write + Send> + Send + Sync> = if console_to_stderr
    {
        Arc::new(|| Box::new(std::io::stderr()))
    } else {
        Arc::new(|| Box::new(std::io::stdout()))
    };

    // File writer (optional).
    let (file_nb, handle): (Option<NonBlocking>, Option<StartupLogHandle>) = if let Some(path) = cfg
        .file_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let p = Path::new(path);

        if let Some(parent) = p.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!(
                    "startup: logging: failed to create log directory '{}': {e}. Falling back to console-only.",
                    parent.display()
                );
                (None, None)
            } else {
                match p
                    .file_name()
                    .and_then(|v| v.to_str())
                    .filter(|s| !s.is_empty())
                {
                    Some(file_name) => {
                        let dir = p.parent().unwrap_or_else(|| Path::new("."));
                        let appender = tracing_appender::rolling::never(dir, file_name);
                        let (nb, guard) = tracing_appender::non_blocking(appender);
                        (Some(nb), Some(StartupLogHandle { _guard: guard }))
                    }
                    None => {
                        eprintln!(
                            "startup: logging: invalid log file name '{}'. Falling back to console-only.",
                            p.display()
                        );
                        (None, None)
                    }
                }
            }
        } else {
            // No parent: current directory.
            match p
                .file_name()
                .and_then(|v| v.to_str())
                .filter(|s| !s.is_empty())
            {
                Some(file_name) => {
                    let appender = tracing_appender::rolling::never(Path::new("."), file_name);
                    let (nb, guard) = tracing_appender::non_blocking(appender);
                    (Some(nb), Some(StartupLogHandle { _guard: guard }))
                }
                None => {
                    eprintln!(
                        "startup: logging: invalid log file name '{}'. Falling back to console-only.",
                        p.display()
                    );
                    (None, None)
                }
            }
        }
    } else {
        (None, None)
    };

    // Prevent move-after-use: compute flags before consuming file_nb.
    let has_file = file_nb.is_some();
    let tee = cfg.tee;

    // Build ONE boxed MakeWriter for all modes.
    let writer = match file_nb {
        Some(nb) => {
            if tee {
                let mk = DynMakeWriter {
                    mk: {
                        let console_mk = Arc::clone(&console_mk);
                        let nb = nb.clone();
                        Arc::new(move || {
                            let a = (console_mk)();
                            let b = Box::new(nb.make_writer());
                            Box::new(TeeWriter { a, b }) as Box<dyn io::Write + Send>
                        })
                    },
                };
                fmt::writer::BoxMakeWriter::new(mk)
            } else {
                let mk = DynMakeWriter {
                    mk: {
                        let nb = nb.clone();
                        Arc::new(move || Box::new(nb.make_writer()) as Box<dyn io::Write + Send>)
                    },
                };
                fmt::writer::BoxMakeWriter::new(mk)
            }
        }
        None => {
            let mk = DynMakeWriter { mk: console_mk };
            fmt::writer::BoxMakeWriter::new(mk)
        }
    };

    // ANSI policy:
    // - file-only: never ANSI
    // - tee / console-only: respect cfg.colors
    let ansi = if has_file && !tee { false } else { cfg.colors };

    let fmt_layer = mk_fmt_layer(&cfg, timer, ansi, writer);

    // IMPORTANT: add fmt_layer first, then filter to avoid E0277 type mismatch.
    let subscriber = tracing_subscriber::registry().with(fmt_layer).with(filter);

    if tracing::subscriber::set_global_default(subscriber).is_err() {
        return Ok(None);
    }

    let _ = LOG_INIT.set(());

    Ok(handle)
}