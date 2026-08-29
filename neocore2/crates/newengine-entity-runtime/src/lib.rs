#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.entity` gateway runtime service.
//!
//! Entity lifecycle calls are exposed through stable DTOs from
//! `newengine-entity-api` and operate against the shared runtime scene bridge.

mod archetype;
mod operations;
mod payload;
mod registration;
mod router;
mod service;

pub use archetype::{
    default_entity_archetype_registry, register_entity_archetype,
    register_entity_archetype_definition, EntityArchetypeFactory, EntityArchetypeRegistry,
};
pub use registration::register_entity_gateway_best_effort;
pub use router::entity_gateway_service;
pub use service::{EngineEntityGatewayService, ENTITY_GATEWAY_OWNER};

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.entity",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_entity_api::ENTITY_BACKEND_CAPABILITY_ID],
        &["ecs.backend"],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    engine: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let scene = engine
        .resources_mut()
        .get::<std::sync::Arc<newengine_scene_runtime::SceneBridge>>()
        .cloned()
        .ok_or_else(|| newengine_runtime_unit_api::EngineError::Other(
            "entity runtime unit requires instance Arc<SceneBridge> resource before materialization".to_owned(),
        ))?;
    register_entity_gateway_best_effort(scene);
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );
