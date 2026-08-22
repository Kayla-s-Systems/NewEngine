#![forbid(unsafe_op_in_unsafe_fn)]

mod cvar;
mod runtime;
mod service;
mod types;

pub use cvar::{
    global_cvar_registry, register_cvar, CVarDescriptor, CVarFlags, CVarHandle, CVarRegistry,
    CVarSnapshot, CVarType, CVarValue,
};
pub use newengine_console_api::{
    method, CommandArgSpec, CommandDescriptor, CommandFlags, COMMAND_BACKEND_CAPABILITY_ID,
    COMMAND_DESCRIPTOR_CONTRACT_ID, COMMAND_PROVIDER_ROUTE, COMMAND_PROVIDER_SERVICE_ID,
    COMMAND_SERVICE_ID, COMMAND_SERVICE_KIND, ENGINE_COMMAND_GATEWAY_ID,
};
pub use service::install_console_provider;
