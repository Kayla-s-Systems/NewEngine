#![forbid(unsafe_op_in_unsafe_fn)]

use env_logger::fmt::{TimestampPrecision, WriteStyle};
use log::LevelFilter;

use std::{env, path::PathBuf};

use crate::logger::output::LogOutput;

#[derive(Debug, Clone)]
pub struct ConsoleLoggerConfig {
    pub filter: Option<String>,
    pub level: LevelFilter,
    pub write_style: Option<WriteStyle>,
    pub colors: bool,
    pub include_module_path: bool,
    pub include_target: bool,
    pub include_file: bool,
    pub include_line_number: bool,
    pub timestamp: Option<TimestampPrecision>,
    pub indent: Option<usize>,

    pub console_output: Option<LogOutput>,

    pub file_path: Option<PathBuf>,
    pub tee: bool,

    /// If set, rotate when file grows beyond this size.
    pub roll_max_bytes: Option<u64>,
    /// Max number of size-rolled backups (path.1..path.N).
    pub roll_max_files: usize,
    /// If set, rotate when UTC day changes; keeps only last N day-files.
    pub roll_keep_days: Option<usize>,

    /// If set (>0), suppress repeated log lines within this time window (milliseconds).
    ///
    /// This is a logger-level protection against per-frame spam (e.g. render telemetry),
    /// and is applied after formatting, so it works for any log producer.
    pub dedup_window_ms: Option<u64>,

    /// Max number of distinct dedup keys tracked at once (LRU).
    pub dedup_capacity: usize,
}

impl Default for ConsoleLoggerConfig {
    #[inline]
    fn default() -> Self {
        Self::from_env()
    }
}

impl ConsoleLoggerConfig {
    pub fn from_env() -> Self {
        let filter = env::var("NEWENGINE_LOG").ok().filter(|s| !s.is_empty());
        let level = match filter {
            Some(_) => LevelFilter::Info,
            None => env::var("NEWENGINE_LOG_LEVEL")
                .ok()
                .as_deref()
                .and_then(|v| v.parse::<LevelFilter>().ok())
                .unwrap_or(LevelFilter::Info),
        };

        let style_env = env::var("NEWENGINE_LOG_STYLE").ok();
        let write_style = match style_env.as_deref().map(str::to_ascii_lowercase) {
            Some(ref s) if s == "always" || s == "true" || s == "1" => Some(WriteStyle::Always),
            Some(ref s) if s == "never" || s == "false" || s == "0" => Some(WriteStyle::Never),
            Some(ref s) if s == "auto" => Some(WriteStyle::Auto),
            _ => None,
        };

        let colors_env = env::var("NEWENGINE_LOG_COLORS")
            .ok()
            .or_else(|| env::var("NEWENGINE_LOG_COLOR").ok());
        let colors = match colors_env.as_deref().map(str::to_ascii_lowercase) {
            Some(ref v) if v == "0" || v == "false" => false,
            Some(_) => true,
            None => true,
        };

        let include_module_path = env::var("NEWENGINE_LOG_MODULE")
            .ok()
            .map(|v| !matches!(v.as_str(), "0" | "false"))
            .unwrap_or(true);
        let include_target = env::var("NEWENGINE_LOG_TARGET_FIELD")
            .ok()
            .map(|v| !matches!(v.as_str(), "0" | "false"))
            .unwrap_or(true);
        let include_file = env::var("NEWENGINE_LOG_FILE")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true"))
            .unwrap_or(false);
        let include_line_number = env::var("NEWENGINE_LOG_LINE")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true"))
            .unwrap_or(false);

        let timestamp = match env::var("NEWENGINE_LOG_TIMESTAMP")
            .ok()
            .map(|v| v.to_ascii_lowercase())
        {
            Some(ref v) if v == "none" || v == "0" || v == "false" => None,
            Some(ref v) if v == "seconds" || v == "sec" || v == "secs" || v == "s" => {
                Some(TimestampPrecision::Seconds)
            }
            Some(ref v) if v == "milliseconds" || v == "millis" || v == "ms" => {
                Some(TimestampPrecision::Millis)
            }
            Some(ref v) if v == "microseconds" || v == "micros" || v == "us" => {
                Some(TimestampPrecision::Micros)
            }
            Some(ref v) if v == "nanoseconds" || v == "nanos" || v == "ns" => {
                Some(TimestampPrecision::Nanos)
            }
            Some(_) => Some(TimestampPrecision::Millis),
            None => Some(TimestampPrecision::Millis),
        };

        let indent = env::var("NEWENGINE_LOG_INDENT")
            .ok()
            .and_then(|v| {
                if v.to_ascii_lowercase() == "none" {
                    Some(None)
                } else {
                    v.parse::<usize>().ok().map(Some)
                }
            })
            .unwrap_or(None);

        let console_output = match env::var("NEWENGINE_LOG_TARGET")
            .ok()
            .map(|v| v.to_ascii_lowercase())
        {
            Some(ref v) if v == "stdout" => Some(LogOutput::Stdout),
            Some(ref v) if v == "stderr" => Some(LogOutput::Stderr),
            _ => None,
        };

        let file_path = env::var("NEWENGINE_LOG_FILE")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "0" && s.to_ascii_lowercase() != "false")
            .map(PathBuf::from);

        let tee = match env::var("NEWENGINE_LOG_TEE").ok().map(|v| v.to_ascii_lowercase()) {
            Some(v) if v == "0" || v == "false" => false,
            Some(_) => true,
            None => file_path.is_some(),
        };

        let roll_max_bytes = env::var("NEWENGINE_LOG_ROLL_MAX_BYTES")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|&v| v > 0);

        let roll_max_files = env::var("NEWENGINE_LOG_ROLL_MAX_FILES")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(5);

        let roll_keep_days = env::var("NEWENGINE_LOG_ROLL_KEEP_DAYS")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|&v| v > 0);

        let dedup_window_ms = env::var("NEWENGINE_LOG_DEDUP_WINDOW_MS")
            .ok()
            .or_else(|| env::var("NEWENGINE_LOG_DEDUP_MS").ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|&v| v > 0);

        let dedup_capacity = env::var("NEWENGINE_LOG_DEDUP_CAPACITY")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(2048);

        Self {
            filter,
            level,
            write_style,
            colors,
            include_module_path,
            include_target,
            include_file,
            include_line_number,
            timestamp,
            indent,
            console_output,
            file_path,
            tee,
            roll_max_bytes,
            roll_max_files,
            roll_keep_days,
            dedup_window_ms,
            dedup_capacity,
        }
    }

    #[inline]
    pub fn effective_console_output(&self) -> LogOutput {
        self.console_output.unwrap_or(LogOutput::Stderr)
    }

    #[inline]
    pub fn rolling_enabled(&self) -> bool {
        self.roll_max_bytes.is_some() || self.roll_keep_days.is_some()
    }
}
