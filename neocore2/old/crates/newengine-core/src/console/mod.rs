#![forbid(unsafe_op_in_unsafe_fn)]

mod method;
mod runtime;
mod service;
mod types;

pub use method::COMMAND_SERVICE_ID;
pub use service::{init_console_service, take_exit_requested};
