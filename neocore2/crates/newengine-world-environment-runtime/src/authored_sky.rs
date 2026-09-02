use std::f32::consts::TAU;

use newengine_bounds::Bounds;
use newengine_core::call_service_v1_optional;
use newengine_ecs::EntityId;
use newengine_engine_runtime::world_authoring::apply_exact_material;
use newengine_lighting::{AmbientLight, DirectionalLight, ShadowSettings};
use newengine_materials::{MaterialId, MaterialRegistry};
use newengine_math::{Vec2, Vec3};
use newengine_model_domain_api::MeshRenderOptions;
use newengine_primitives::{fnv1a_64, Primitive, PrimitiveId};
use newengine_scene::spawn_named;
use newengine_transform::{set_parent, Transform};
use newengine_world_environment_api::authored_profile::{
    AuthoredDayNightSpec as GameReadyDayNightSpec, AuthoredLightingSpec as GameReadyLightingSpec,
    AuthoredSkyAtmosphereSpec as GameReadySkyAtmosphereSpec, AuthoredSkySpec as GameReadySkySpec,
};

// Sky is split by responsibility. The facade preserves the historical module
// surface while keeping atmosphere data, sampling, runtime application and
// lighting bootstrap independently testable.
#[path = "authored_sky/sky_clouds.rs"]
mod sky_clouds;
#[path = "authored_sky/sky_lighting.rs"]
mod sky_lighting;
#[path = "authored_sky/sky_postfx.rs"]
mod sky_postfx;
#[path = "authored_sky/sky_runtime.rs"]
mod sky_runtime;
#[path = "authored_sky/sky_sampling.rs"]
mod sky_sampling;
#[path = "authored_sky/sky_types.rs"]
mod sky_types;

// Private imports make sibling modules and tests consume one stable sky facade.
use self::sky_clouds::*;
use self::sky_postfx::*;
use self::sky_sampling::*;

pub use self::sky_lighting::configure_game_ready_lighting;
pub use self::sky_lighting::configure_game_ready_lighting as configure_authored_lighting;
pub use self::sky_runtime::tick_game_ready_sky_cycle;
pub use self::sky_runtime::tick_game_ready_sky_cycle as tick_authored_sky_cycle;
pub use self::sky_types::{
    attach_sky_visual_runtime, sky_atmosphere_from_spec, SkyDynamicsFrame, SkyFrameSample,
    SKY_VISUAL_SPAWN_ORDER,
};
pub use self::sky_types::{
    CloudSunOcclusionRuntime, GameReadyEnvironmentVisualAssetsRuntime, SkyAtmosphereRuntime,
    SkyCycleRuntime, SkyDynamicsRuntime, SkyVisualKind, SkyVisualRuntime,
};
pub(crate) use newengine_engine_runtime::gameplay::{
    CloudShadowRenderState, EnvironmentDomeRenderState, EnvironmentPostFxState, WorldClearColor,
};

pub const AUTHORED_SKYDOME_PRIMITIVE_ID: PrimitiveId =
    PrimitiveId(fnv1a_64("kalitech.asset.skydome.high.v1"));
