#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.world` gateway service.
//!
//! `engine.scene` is authored structure while `engine.world` is the living
//! runtime instance. The gateway exposes DTOs only.

mod apply_stage;
mod bookkeeping;
mod invoke;
mod partition;
mod payload;
mod registration;
mod router;
mod service;
mod snapshot;
mod state;
mod streaming_cells;

pub use registration::register_world_gateway_best_effort;
pub use router::world_gateway_service;
pub use service::{
    EngineWorldGatewayService, WORLD_FOUNDATION_PROVIDER_ROUTE, WORLD_GATEWAY_OWNER,
};

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.world",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_world_api::WORLD_BACKEND_CAPABILITY_ID],
        &["scene.backend"],
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
            "world runtime unit requires instance Arc<SceneBridge> resource before materialization".to_owned(),
        ))?;
    register_world_gateway_best_effort(scene);
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );
