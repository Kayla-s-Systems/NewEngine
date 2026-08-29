#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted provider routes for `engine.world.environment`.
//!
//! Providers expose resolved environment DTOs through the gateway registry and
//! do not mutate ECS/world storage or inspect renderer/GPU state.

mod celestial;
mod constants;
mod consumer_packets;
mod default_provider;
mod math;
mod payload;
mod profile_catalog;
mod provider_state;
mod registration;
mod router;
mod visual_asset_catalog;

pub use constants::{
    WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE, WORLD_ENVIRONMENT_GATEWAY_OWNER,
    WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE, WORLD_ENVIRONMENT_SNAPSHOT_SCHEMA_V1,
};
pub use registration::register_world_environment_gateway_best_effort;

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.world-environment",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_world_environment_api::WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID],
        &[newengine_world_api::WORLD_BACKEND_CAPABILITY_ID],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    register_world_environment_gateway_best_effort();
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );

#[cfg(test)]
mod tests;
