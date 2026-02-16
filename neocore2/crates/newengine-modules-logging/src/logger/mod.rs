#![forbid(unsafe_op_in_unsafe_fn)]

pub mod config;
pub mod module;
pub mod output;
pub mod sink;

pub use config::ConsoleLoggerConfig;
pub use module::ConsoleLoggerModule;
pub use output::LogOutput;
