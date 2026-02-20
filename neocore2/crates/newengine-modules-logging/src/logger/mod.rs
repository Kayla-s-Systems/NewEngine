#![forbid(unsafe_op_in_unsafe_fn)]

pub mod config;
pub mod init;
pub mod output;
pub mod sink;

pub use config::ConsoleLoggerConfig;
pub use init::init_console_logger;
pub use output::LogOutput;
