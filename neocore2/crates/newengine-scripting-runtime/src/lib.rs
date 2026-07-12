#![forbid(unsafe_op_in_unsafe_fn)]

mod codec;
mod constants;
mod service;
mod state;
mod validation;

pub use service::{register_scripting_gateway_best_effort, scripting_gateway_service};
pub use state::{ScriptingRuntimeServiceInfo, ScriptingRuntimeState};
pub use validation::validate_script_module_ref;

#[cfg(test)]
mod tests;
