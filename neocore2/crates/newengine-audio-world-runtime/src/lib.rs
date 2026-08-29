#![forbid(unsafe_op_in_unsafe_fn)]

//! Bounded world-facing audio runtime.
//!
//! This crate owns spatial audio scene/ambience lifecycle and the stable `engine.audio`
//! gateway bridge. It deliberately consumes a narrow shared scene resource instead of
//! depending on the engine-runtime composition crate.

use std::sync::Arc;

use parking_lot::RwLock;

mod audio_ambience;
mod audio_environment;
mod audio_gateway_fallback;
mod audio_scene;

pub use audio_ambience::AudioAmbienceRuntimeModule;
pub use audio_environment::{
    AudioEnvironmentFrame, AudioEnvironmentResolution, AudioEnvironmentRuntimeState,
};
pub use audio_gateway_fallback::register_audio_gateway_best_effort;
pub use audio_scene::{
    AcousticSurface, AudioEmitter, AudioEnvironmentZone, AudioPortal, AudioSceneRuntimeModule,
};
pub use newengine_audio_api::AudioAmbienceBed;

/// Narrow instance-local scene resource consumed by world-audio runtime units.
/// Product/profile composition publishes it before runtime-unit materialization.
#[derive(Clone)]
pub struct AudioWorldScene {
    scene: Arc<RwLock<newengine_scene::Scene>>,
}

impl AudioWorldScene {
    #[inline]
    pub fn new(scene: Arc<RwLock<newengine_scene::Scene>>) -> Self {
        Self { scene }
    }

    #[inline]
    pub fn scene(&self) -> Arc<RwLock<newengine_scene::Scene>> {
        Arc::clone(&self.scene)
    }
}

pub use newengine_audio_world_api::{
    AudioAmbienceBedRuntime, AudioEarlyReflectionObservation, AudioEarlyReflectionPathObservation,
    AudioEmitterRuntime, AudioListenerRuntimeState, AudioOcclusionObservation,
};

pub const AUDIO_SCENE_RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.audio-scene",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Module,
        &["engine.runtime.audio-scene"],
        &[
            "scene.backend",
            newengine_audio_api::AUDIO_BACKEND_CAPABILITY_ID,
        ],
        newengine_runtime_unit_api::STATIC_MODULE_TAGS,
    );

pub const AUDIO_AMBIENCE_RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.audio-ambience",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Module,
        &["engine.runtime.audio-ambience"],
        &[
            "scene.backend",
            newengine_audio_api::AUDIO_BACKEND_CAPABILITY_ID,
        ],
        newengine_runtime_unit_api::STATIC_MODULE_TAGS,
    );

fn runtime_unit_scene(
    engine: &mut newengine_runtime_unit_api::Engine<()>,
) -> newengine_runtime_unit_api::EngineResult<AudioWorldScene> {
    engine
        .resources_mut()
        .get::<AudioWorldScene>()
        .cloned()
        .ok_or_else(|| {
            newengine_runtime_unit_api::EngineError::Other(
                "world-audio runtime unit requires AudioWorldScene resource before materialization"
                    .to_owned(),
            )
        })
}

fn audio_scene_runtime_unit_factory(
    engine: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    register_audio_gateway_best_effort();
    Ok(Some(Box::new(AudioSceneRuntimeModule::new(
        runtime_unit_scene(engine)?,
    ))))
}

fn audio_ambience_runtime_unit_factory(
    engine: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    register_audio_gateway_best_effort();
    Ok(Some(Box::new(AudioAmbienceRuntimeModule::new(
        runtime_unit_scene(engine)?,
    ))))
}

pub const AUDIO_SCENE_RUNTIME_UNIT_REGISTRATION:
    newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        AUDIO_SCENE_RUNTIME_UNIT_SPEC,
        audio_scene_runtime_unit_factory,
    );

pub const AUDIO_AMBIENCE_RUNTIME_UNIT_REGISTRATION:
    newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        AUDIO_AMBIENCE_RUNTIME_UNIT_SPEC,
        audio_ambience_runtime_unit_factory,
    );
