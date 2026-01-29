use env_logger::Builder;
use log::LevelFilter;
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};

use std::io::Write;

#[derive(Debug, Clone)]
pub struct ConsoleLoggerConfig {
    pub level: LevelFilter,
    pub colors: bool,
    pub include_module: bool,
}

impl ConsoleLoggerConfig {
    pub fn from_env() -> Self {
        let level = std::env::var("NEWENGINE_LOG")
            .ok()
            .and_then(|v| v.parse::<LevelFilter>().ok())
            .unwrap_or(LevelFilter::Info);

        let colors = std::env::var("NEWENGINE_LOG_COLORS")
            .ok()
            .map(|v| v != "0")
            .unwrap_or(true);

        let include_module = std::env::var("NEWENGINE_LOG_MODULE")
            .ok()
            .map(|v| v != "0")
            .unwrap_or(true);

        Self {
            level,
            colors,
            include_module,
        }
    }
}

impl Default for ConsoleLoggerConfig {
    fn default() -> Self {
        Self::from_env()
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

impl<E: Send + 'static> Module<E> for ConsoleLoggerModule {
    fn id(&self) -> &'static str {
        "console-logger"
    }

    fn init(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if self.initialized {
            return Ok(());
        }

        let mut builder = Builder::new();
        builder.filter_level(self.config.level);

        let config = self.config.clone();

        builder.format(move |buf, record| {
            let level_str = record.level().as_str();
            let reset = if config.colors { "\x1b[0m" } else { "" };

            let level_prefix = if config.colors {
                match record.level() {
                    log::Level::Error => "\x1b[1;31m", // bold red
                    log::Level::Warn => "\x1b[1;33m",  // bold yellow
                    log::Level::Info => "\x1b[0;32m",  // green
                    log::Level::Debug => "\x1b[0;34m", // blue
                    log::Level::Trace => "\x1b[0;35m", // magenta
                }
            } else {
                ""
            };

            if config.include_module {
                writeln!(
                    buf,
                    "[{}{: <5}{}] {: <25} {}",
                    level_prefix,
                    level_str,
                    reset,
                    record.target(),
                    record.args()
                )
            } else {
                writeln!(
                    buf,
                    "[{}{: <5}{}] {}",
                    level_prefix,
                    level_str,
                    reset,
                    record.args()
                )
            }
        });

        builder
            .try_init()
            .map_err(|e| EngineError::Other(format!("logger init failed: {e}")))?;

        self.initialized = true;
        Ok(())
    }
}