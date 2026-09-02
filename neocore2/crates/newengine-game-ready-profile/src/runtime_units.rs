#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::{Engine, EngineError, EngineResult, Module, StartupConfig};
use newengine_game_data::GameDataProvider;
use newengine_game_data_lua::{
    LuaGameDataProvider, LUA_GAME_DATA_PROVIDER_ID, SCRIPT_GAME_DATA_PROVIDER_ID,
};
use newengine_runtime_host::app_launcher::RuntimeHostRuntimeUnitRegistration;
use newengine_service_api::{EngineRuntimeUnitKind, EngineRuntimeUnitSpec};

use crate::scene_bootstrap::{
    GameReadyWorldSceneBootstrapProvider, ProjectAudioMixBootstrapCompletion,
};
use crate::world_runtime::GameReadyWorldRuntimeProvider;

const GAME_READY_UNIT_TAGS: &[&str] = &[
    "engine.runtime-unit",
    "game-ready",
    "first-party",
    "game-module-contribution",
];

pub(crate) const GAME_READY_SCENE_BOOTSTRAP_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec =
    EngineRuntimeUnitSpec::new(
        "newengine.gameready.scene-bootstrap",
        1,
        EngineRuntimeUnitKind::Module,
        &[newengine_service_api::runtime_unit_capability::GAME_SCENE_BOOTSTRAP],
        &[
            newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID,
            newengine_scripting_api::SCRIPTING_BACKEND_CAPABILITY_ID,
        ],
        GAME_READY_UNIT_TAGS,
    );

pub(crate) const GAME_READY_WORLD_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec =
    EngineRuntimeUnitSpec::new(
        "newengine.gameready.world-runtime",
        1,
        EngineRuntimeUnitKind::Provider,
        &[newengine_service_api::runtime_unit_capability::GAME_WORLD_RUNTIME],
        &[newengine_world_api::WORLD_BACKEND_CAPABILITY_ID],
        GAME_READY_UNIT_TAGS,
    );

pub(crate) struct GameReadyGameDataProviderOverride(pub Arc<dyn GameDataProvider>);

fn runtime_context(
    engine: &mut Engine<()>,
) -> EngineResult<newengine_project_runtime::RuntimeCompositionContext> {
    engine
        .resources_mut()
        .get::<newengine_project_runtime::RuntimeCompositionContext>()
        .cloned()
        .ok_or_else(|| {
            EngineError::Other(
                "GameReady composition runtime unit requires RuntimeCompositionContext".to_owned(),
            )
        })
}

fn resolve_game_data_provider(engine: &mut Engine<()>) -> EngineResult<Arc<dyn GameDataProvider>> {
    let runtime = runtime_context(engine)?;
    if let Some((binding_id, binding)) = runtime
        .scripts
        .binding(SCRIPT_GAME_DATA_PROVIDER_ID)
        .map(|binding| (SCRIPT_GAME_DATA_PROVIDER_ID, binding))
        .or_else(|| {
            runtime
                .scripts
                .binding(LUA_GAME_DATA_PROVIDER_ID)
                .map(|binding| (LUA_GAME_DATA_PROVIDER_ID, binding))
        })
    {
        let operation = binding.operation.ok_or_else(|| {
            EngineError::Other(format!(
                "runtime scripting binding '{}' must declare an operation",
                binding_id
            ))
        })?;
        return Ok(Arc::new(
            LuaGameDataProvider::new(binding.script_ref).with_operation(operation),
        ));
    }
    if let Some(provider) = engine
        .resources_mut()
        .get::<GameReadyGameDataProviderOverride>()
        .map(|value| Arc::clone(&value.0))
    {
        return Ok(provider);
    }
    Err(EngineError::Other(format!(
        "GameReady scene-bootstrap unit requires project-authored game data binding '{}' (legacy '{}', or an explicitly injected test/tool provider); built-in game-data fallback is forbidden",
        SCRIPT_GAME_DATA_PROVIDER_ID,
        LUA_GAME_DATA_PROVIDER_ID
    )))
}

fn scene_bootstrap_factory(
    engine: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let runtime = runtime_context(engine)?;
    if !runtime
        .startup_scene
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(EngineError::Other(
            "GameReady scene-bootstrap runtime unit requires startup_scene from game.toml or its packaged game descriptor"
                .to_owned(),
        ));
    }
    let game_data_provider = resolve_game_data_provider(engine)?;
    let scene = engine
        .resources_mut()
        .get::<Arc<newengine_scene_runtime::SceneBridge>>()
        .cloned()
        .ok_or_else(|| {
            EngineError::Other(
            "GameReady scene-bootstrap runtime unit requires instance Arc<SceneBridge> resource"
                .to_owned(),
        )
        })?;
    scene.set_scene_bootstrap_provider(GameReadyWorldSceneBootstrapProvider::shared(
        game_data_provider,
    ));
    Ok(Some(Box::new(
        newengine_authored_world_runtime::AuthoredWorldBootstrapModule::new(scene)
            .with_completion(Arc::new(ProjectAudioMixBootstrapCompletion)),
    )))
}

fn render_contributions_mut(
    engine: &mut Engine<()>,
) -> &mut newengine_engine_runtime::RuntimeRenderContributionRegistry {
    if engine
        .resources_mut()
        .get::<newengine_engine_runtime::RuntimeRenderContributionRegistry>()
        .is_none()
    {
        engine
            .resources_mut()
            .insert(newengine_engine_runtime::RuntimeRenderContributionRegistry::new());
    }
    engine
        .resources_mut()
        .get_mut::<newengine_engine_runtime::RuntimeRenderContributionRegistry>()
        .expect("runtime render contribution registry inserted")
}

fn world_runtime_factory(
    engine: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    render_contributions_mut(engine)
        .register_world_runtime_provider(GameReadyWorldRuntimeProvider::shared());
    Ok(None)
}

fn input_profile_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    newengine_input_bindings_runtime::register_input_bindings_gateway_best_effort(
        newengine_input_profile_gameready::game_ready_game_input_profile(),
    );
    Ok(None)
}

fn render_feature_factory(
    engine: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    render_contributions_mut(engine)
        .register_render_feature(
            newengine_render_feature_gameready::GameReadyRenderFeaturePack::new()
                .runtime_contribution(),
        )
        .map_err(EngineError::Other)?;
    Ok(None)
}

pub(crate) const GAME_READY_RUNTIME_UNIT_REGISTRATIONS: &[RuntimeHostRuntimeUnitRegistration] = &[
    RuntimeHostRuntimeUnitRegistration::new(
        GAME_READY_SCENE_BOOTSTRAP_RUNTIME_UNIT_SPEC,
        scene_bootstrap_factory,
    ),
    RuntimeHostRuntimeUnitRegistration::new(
        GAME_READY_WORLD_RUNTIME_UNIT_SPEC,
        world_runtime_factory,
    ),
    RuntimeHostRuntimeUnitRegistration::new(
        newengine_input_profile_gameready::GAME_READY_INPUT_PROFILE_RUNTIME_UNIT_SPEC,
        input_profile_factory,
    ),
    RuntimeHostRuntimeUnitRegistration::new(
        newengine_render_feature_gameready::GAME_READY_RENDER_FEATURE_RUNTIME_UNIT_SPEC,
        render_feature_factory,
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_ready_units_produce_all_canonical_game_module_capabilities() {
        let provided = GAME_READY_RUNTIME_UNIT_REGISTRATIONS
            .iter()
            .flat_map(|registration| registration.spec.provides.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();
        for capability in [
            newengine_service_api::runtime_unit_capability::GAME_SCENE_BOOTSTRAP,
            newengine_service_api::runtime_unit_capability::GAME_WORLD_RUNTIME,
            newengine_service_api::runtime_unit_capability::GAME_INPUT_PROFILE,
            newengine_service_api::runtime_unit_capability::RENDER_FEATURE,
        ] {
            assert!(
                provided.contains(capability),
                "missing producer for {capability}"
            );
        }
    }
}
