#![forbid(unsafe_op_in_unsafe_fn)]

use env_logger::fmt::{Target, TimestampPrecision, WriteStyle};
use env_logger::Builder;
use newengine_core::{EngineResult, Module, ModuleCtx};

use crate::logger::{
    config::ConsoleLoggerConfig,
    sink::{LockedFileWriter, RollingConfig, RollingFileWriter, TeeWriter},
};

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

    fn build_logger(&self) -> Builder {
        let mut builder = Builder::new();

        if let Some(ref filters) = self.config.filter {
            builder.parse_filters(filters);
        } else {
            builder.filter_level(self.config.level);
        }

        if let Some(path) = self.config.file_path.clone() {
            let file_writer: Option<Box<dyn std::io::Write + Send>> = if self.config.rolling_enabled()
            {
                let rcfg = RollingConfig {
                    max_bytes: self.config.roll_max_bytes,
                    max_files: self.config.roll_max_files,
                    keep_days: self.config.roll_keep_days,
                };
                match RollingFileWriter::open_append(path, rcfg) {
                    Ok(w) => Some(Box::new(w)),
                    Err(_) => None,
                }
            } else {
                match LockedFileWriter::open_append(path) {
                    Ok(w) => Some(Box::new(w)),
                    Err(_) => None,
                }
            };

            if let Some(w) = file_writer {
                if self.config.tee {
                    let console = self.config.effective_console_output();
                    let tee = TeeWriter::new(console, w);
                    builder.target(Target::Pipe(Box::new(tee)));
                } else {
                    builder.target(Target::Pipe(w));
                }
            } else {
                builder.target(self.config.effective_console_output().to_env_target());
            }
        } else {
            builder.target(self.config.effective_console_output().to_env_target());
        }

        let file_active = self.config.file_path.is_some();
        if let Some(style) = self.config.write_style {
            builder.write_style(style);
        } else if file_active || !self.config.colors {
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

        builder
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

        let builder = self.build_logger();
        let _ = builder.try_init();

        self.initialized = true;
        Ok(())
    }
}
