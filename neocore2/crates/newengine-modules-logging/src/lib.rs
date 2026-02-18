#![forbid(unsafe_op_in_unsafe_fn)]

pub mod logger;

pub use logger::{ConsoleLoggerConfig, ConsoleLoggerModule, LogOutput};


/// Installs the global logger immediately.
///
/// This is intended for early bootstrap (before Engine::start and plugin loading).
pub fn install_global(cfg: &ConsoleLoggerConfig) -> Result<(), log::SetLoggerError> {
    let mut module = ConsoleLoggerModule::new(cfg.clone());
    module.install_now()
}
