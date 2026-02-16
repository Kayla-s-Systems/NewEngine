#![forbid(unsafe_op_in_unsafe_fn)]

pub mod logger;

pub use logger::{ConsoleLoggerConfig, ConsoleLoggerModule, LogOutput};
