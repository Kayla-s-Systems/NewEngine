#![forbid(unsafe_op_in_unsafe_fn)]

mod codec;
mod constants;
mod service;
mod state;
mod validation;

pub use service::{register_scripting_gateway_best_effort, scripting_gateway_service};
pub use state::{ScriptingRuntimeServiceInfo, ScriptingRuntimeState};
pub use validation::validate_script_module_ref;

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.scripting",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_scripting_api::SCRIPTING_BACKEND_CAPABILITY_ID],
        &[],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let _ = register_scripting_gateway_best_effort();
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );

#[cfg(test)]
mod tests;
