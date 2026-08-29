#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.scene` gateway runtime service.
//!
//! Product profiles choose whether to register this service, but do not own its
//! scene IO transport, authored-scene queries or gateway metadata.

mod asset_io;
mod constants;
mod instantiation;
mod queries;
mod registration;
mod state;
mod transport;
mod validation;

pub use constants::SCENE_GATEWAY_OWNER;
pub use newengine_engine_runtime::SceneBridge;
pub use registration::{register_scene_gateway_best_effort, scene_gateway_service};
pub use state::{EngineSceneGatewayService, SceneGatewayAssetMounts};

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.scene",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID],
        &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    engine: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let scene = engine
        .resources_mut()
        .get::<std::sync::Arc<SceneBridge>>()
        .cloned()
        .ok_or_else(|| newengine_runtime_unit_api::EngineError::Other(
            "scene runtime unit requires instance Arc<SceneBridge> resource before materialization".to_owned(),
        ))?;
    let mounts = engine
        .resources_mut()
        .get::<SceneGatewayAssetMounts>()
        .copied();
    register_scene_gateway_best_effort(scene, mounts);
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );
