#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

/// Holds guard to keep file writer alive.
pub struct LogHandle {
    _guard: WorkerGuard,
}

/// Initializes engine logging.
/// Safe to call once during startup.
/// Returns LogHandle that must be kept alive.
pub fn init_logging(
    level: &str,
    log_file: Option<&str>,
) -> Result<Option<LogHandle>, Box<dyn std::error::Error>> {
    let filter = EnvFilter::try_new(level)
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);

    if let Some(file_path) = log_file {
        let path = Path::new(file_path);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or("Invalid log file name")?;

        let dir = path.parent().unwrap_or(Path::new("."));

        let file_appender = rolling::never(dir, file_name);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let file_layer = fmt::layer()
            .with_ansi(false)
            .with_writer(non_blocking)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true);

        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .with(file_layer);

        tracing::subscriber::set_global_default(subscriber)?;

        Ok(Some(LogHandle { _guard: guard }))
    } else {
        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer);

        tracing::subscriber::set_global_default(subscriber)?;

        Ok(None)
    }
}