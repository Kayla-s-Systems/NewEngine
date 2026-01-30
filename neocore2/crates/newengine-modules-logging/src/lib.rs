use env_logger::fmt::{Target, TimestampPrecision, WriteStyle};
use env_logger::Builder;
use log::LevelFilter;
use newengine_core::{EngineResult, Module, ModuleCtx};

use std::env;

/// Logger output destination that is trivially cloneable.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LogOutput {
    Stdout,
    Stderr,
}

impl LogOutput {
    #[inline]
    fn to_env_target(self) -> Target {
        match self {
            LogOutput::Stdout => Target::Stdout,
            LogOutput::Stderr => Target::Stderr,
        }
    }
}

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
    /// Destination for log output. When `None` defaults to `stderr`.
    pub output: Option<LogOutput>,
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

        let output = match env::var("NEWENGINE_LOG_TARGET")
            .ok()
            .map(|v| v.to_ascii_lowercase())
        {
            Some(ref v) if v == "stdout" => Some(LogOutput::Stdout),
            Some(ref v) if v == "stderr" => Some(LogOutput::Stderr),
            _ => None,
        };

        ConsoleLoggerConfig {
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
            output,
        }
    }
}

pub struct ConsoleLoggerModule {
    config: ConsoleLoggerConfig,
    initialized: bool,
}

impl ConsoleLoggerModule {
    #[inline]
    pub fn new(config: ConsoleLoggerConfig) -> Self {
        Self {
            config,
            initialized: false,
        }
    }
}

impl Default for ConsoleLoggerConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl<E: Send + 'static> Module<E> for ConsoleLoggerModule {
    fn id(&self) -> &'static str {
        "console-logger"
    }

    fn init(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if self.initialized {
            return Ok(());
        }

        let mut builder = Builder::new();

        if let Some(ref filters) = self.config.filter {
            builder.parse_filters(filters);
        } else {
            builder.filter_level(self.config.level);
        }

        if let Some(out) = self.config.output {
            builder.target(out.to_env_target());
        }

        if let Some(style) = self.config.write_style {
            builder.write_style(style);
        } else if !self.config.colors {
            builder.write_style(WriteStyle::Never);
        } else {
            builder.write_style(WriteStyle::Auto);
        }

        builder
            .format_module_path(self.config.include_module_path)
            .format_target(self.config.include_target);

        if self.config.include_file && self.config.include_line_number {
            builder.format_source_path(true);
        } else {
            builder.format_file(self.config.include_file);
            builder.format_line_number(self.config.include_line_number);
        }

        builder.format_indent(self.config.indent);

        match self.config.timestamp {
            Some(TimestampPrecision::Seconds) => builder.format_timestamp_secs(),
            Some(TimestampPrecision::Millis) => builder.format_timestamp_millis(),
            Some(TimestampPrecision::Micros) => builder.format_timestamp_micros(),
            Some(TimestampPrecision::Nanos) => builder.format_timestamp_nanos(),
            None => builder.format_timestamp(None::<TimestampPrecision>),
        };

        match builder.try_init() {
            Ok(()) => {}
            Err(_e) => {
                // Most likely "logger already initialized". Treat as non-fatal.
            }
        }

        self.initialized = true;
        Ok(())
    }
}