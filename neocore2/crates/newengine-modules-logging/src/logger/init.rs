#![forbid(unsafe_op_in_unsafe_fn)]

use env_logger::fmt::{Target, TimestampPrecision, WriteStyle};
use env_logger::{Builder, Logger};

use crate::logger::{
    config::ConsoleLoggerConfig,
    sink::{
        ConsoleWriter, DedupWriter, LockedFileWriter, RollingConfig, RollingFileWriter, TeeWriter,
    },
};

/// Builds an `env_logger::Logger` instance configured for NewEngine.
///
/// The returned logger is NOT installed globally. The host process owns the global `log`
/// backend and forwards records into the logging plugin via a service.
pub fn build_env_logger(cfg: &ConsoleLoggerConfig) -> Result<Logger, String> {
    Ok(build_logger(cfg).build())
}

fn build_logger(cfg: &ConsoleLoggerConfig) -> Builder {
    let mut builder = Builder::new();

    if let Some(ref filters) = cfg.filter {
        builder.parse_filters(filters);
    } else {
        builder.filter_level(cfg.level);
    }

    if let Some(path) = cfg.file_path.clone() {
        let file_writer: Option<Box<dyn std::io::Write + Send>> = if cfg.rolling_enabled() {
            let rcfg = RollingConfig {
                max_bytes: cfg.roll_max_bytes,
                max_files: cfg.roll_max_files,
                keep_days: cfg.roll_keep_days,
            };
            RollingFileWriter::open_append(path, rcfg)
                .ok()
                .map(|w| Box::new(w) as Box<dyn std::io::Write + Send>)
        } else {
            LockedFileWriter::open_append(path)
                .ok()
                .map(|w| Box::new(w) as Box<dyn std::io::Write + Send>)
        };

        if let Some(w) = file_writer {
            if cfg.tee {
                let console = cfg.effective_console_output();
                let tee: Box<dyn std::io::Write + Send> = if let Some(ms) = cfg.dedup_window_ms {
                    Box::new(DedupWriter::new(
                        TeeWriter::new(console, w),
                        std::time::Duration::from_millis(ms),
                        cfg.dedup_capacity,
                    ))
                } else {
                    Box::new(TeeWriter::new(console, w))
                };

                builder.target(Target::Pipe(tee));
            } else {
                let out: Box<dyn std::io::Write + Send> = if let Some(ms) = cfg.dedup_window_ms {
                    Box::new(DedupWriter::new(
                        w,
                        std::time::Duration::from_millis(ms),
                        cfg.dedup_capacity,
                    ))
                } else {
                    w
                };

                builder.target(Target::Pipe(out));
            }
        } else {
            builder.target(cfg.effective_console_output().to_env_target());
        }
    } else if let Some(ms) = cfg.dedup_window_ms {
        let console = cfg.effective_console_output();
        let w = DedupWriter::new(
            ConsoleWriter::new(console),
            std::time::Duration::from_millis(ms),
            cfg.dedup_capacity,
        );
        builder.target(Target::Pipe(Box::new(w)));
    } else {
        builder.target(cfg.effective_console_output().to_env_target());
    }

    let file_active = cfg.file_path.is_some();
    if let Some(style) = cfg.write_style {
        builder.write_style(style);
    } else if !cfg.colors {
        builder.write_style(WriteStyle::Never);
    } else if file_active && !cfg.tee {
        // File-only output: keep it clean (no ANSI).
        builder.write_style(WriteStyle::Never);
    } else {
        // Console present (direct or tee): allow colors.
        builder.write_style(WriteStyle::Auto);
    }

    builder
        .format_module_path(cfg.include_module_path)
        .format_target(cfg.include_target);

    if cfg.include_file && cfg.include_line_number {
        builder.format_source_path(true);
    } else {
        builder.format_file(cfg.include_file);
        builder.format_line_number(cfg.include_line_number);
    }

    builder.format_indent(cfg.indent);

    match cfg.timestamp {
        Some(TimestampPrecision::Seconds) => builder.format_timestamp_secs(),
        Some(TimestampPrecision::Millis) => builder.format_timestamp_millis(),
        Some(TimestampPrecision::Micros) => builder.format_timestamp_micros(),
        Some(TimestampPrecision::Nanos) => builder.format_timestamp_nanos(),
        None => builder.format_timestamp(None::<TimestampPrecision>),
    };

    builder
}
