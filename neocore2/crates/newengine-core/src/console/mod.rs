#![forbid(unsafe_op_in_unsafe_fn)]

mod cvar;
mod descriptor;
mod method;
mod runtime;
mod service;
mod types;

pub use cvar::{
    global_cvar_registry, register_cvar, CVarDescriptor, CVarFlags, CVarHandle, CVarRegistry,
    CVarSnapshot, CVarType, CVarValue,
};
pub use descriptor::{CommandArgSpec, CommandDescriptor, CommandFlags};
pub use method::COMMAND_SERVICE_ID;
pub use service::init_console_service;
