use super::*;
use newengine_math::Vec2;
use newengine_model_domain_api::MeshRenderOptions;

// Sky is split by responsibility. The facade preserves the historical module
// surface while keeping atmosphere data, sampling, runtime application and
// lighting bootstrap independently testable.
mod sky_clouds;
#[path = "sky_lighting.rs"]
mod sky_lighting;
mod sky_postfx;
#[path = "sky_runtime.rs"]
mod sky_runtime;
#[path = "sky_sampling.rs"]
mod sky_sampling;
#[path = "sky_types.rs"]
mod sky_types;

// Private imports make sibling modules and tests consume one stable sky facade.
use self::sky_clouds::*;
use self::sky_postfx::*;
use self::sky_sampling::*;

pub(crate) use self::sky_lighting::configure_game_ready_lighting;
pub use self::sky_runtime::tick_game_ready_sky_cycle;
pub(crate) use self::sky_types::{
    attach_sky_visual_runtime, sky_atmosphere_from_spec, SkyDynamicsFrame, SkyFrameSample,
    SKY_VISUAL_SPAWN_ORDER,
};
pub(crate) use self::sky_types::{
    CloudSunOcclusionRuntime, GameReadyEnvironmentVisualAssetsRuntime, SkyAtmosphereRuntime,
    SkyCycleRuntime, SkyDynamicsRuntime, SkyVisualKind, SkyVisualRuntime,
};
pub(crate) use newengine_engine_runtime::gameplay::{
    CloudShadowRenderState, EnvironmentDomeRenderState, EnvironmentPostFxState, WorldClearColor,
};

#[cfg(test)]
mod sky_tests;
