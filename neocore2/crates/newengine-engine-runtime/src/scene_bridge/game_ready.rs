#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "game_ready_parts/assets_bootstrap.rs"]
mod assets_bootstrap;
mod content;
#[path = "game_ready_parts/foliage.rs"]
mod foliage;
#[path = "game_ready_parts/material_source.rs"]
mod material_source;
#[path = "game_ready_parts/materials_terrain.rs"]
mod materials_terrain;
#[path = "game_ready_parts/player_model.rs"]
mod player_model;
#[path = "game_ready_parts/sky.rs"]
mod sky;
#[path = "game_ready_parts/terrain_streaming.rs"]
mod terrain_streaming;
#[path = "game_ready_parts/ytyp_metadata.rs"]
mod ytyp_metadata;

use core::f32::consts::TAU;

use newengine_assets::{wait_ready, AssetServiceClient};
use newengine_bounds::Bounds;
use newengine_core::{
    call_service_v1_optional, JobLane, JobPriority, JobRequest, JobSystemHandle, JobTicket,
};
use newengine_ecs::EntityId;
use newengine_jobs_api::{job_domain, job_pass};
use newengine_lighting::{AmbientLight, DirectionalLight, ShadowSettings};
use newengine_materials::{MaterialFlags, MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_plugin_host::default_host_api;
use newengine_primitives::{
    fnv1a_64, Primitive, PrimitiveId, PrimitiveMesh, PrimitiveRegistry, PrimitiveVertex,
};
use newengine_procedural_noise::{
    DomainWarp2D, NoiseAlgorithm, NoiseCombineMode, NoiseDomain2D, NoiseGraph2D, NoiseLayer2D,
    NoiseRemap, NoiseShape, ProceduralTerrain, TerrainHeightfieldDescriptor,
};
use newengine_scene::{
    spawn_named, Scene, SceneBucketedCellPlan, SceneCellCoord, SceneLayeredStreamingPlan,
    SceneResidencySet, SceneStreamingBudget, SceneStreamingProfile,
};
use newengine_transform::{set_parent, Transform};

use std::sync::{Arc, Mutex};

use crate::gameplay::{
    spawn_player_controller_with_tuning, FpsDemoRules, FpsDemoState, FpsPlayerTuning,
    GameReadyWorldLaunchGate,
};
use crate::scene_bootstrap::bootstrap_runtime_scene;

use self::content::{
    load_game_ready_map_profile, GameReadyDayNightSpec, GameReadyDefinitionApplyMode,
    GameReadyDefinitionInstanceSpec, GameReadyFoliageSpec, GameReadyGameplaySpec,
    GameReadyLightingSpec, GameReadyMapProfile, GameReadyMaterialSetSpec, GameReadyMaterialSpec,
    GameReadyPaletteSpec, GameReadyPrefabSpec, GameReadySkyAtmosphereSpec, GameReadySkySpec,
    GameReadyTerrainSpec,
};
use super::helpers::{
    apply_exact_material, apply_primitive_instance, ensure_primitive_base, ensure_root,
    primitive_bounds,
};

pub(super) use self::assets_bootstrap::bootstrap_fps_game_ready_scene;
pub(crate) use self::sky::{
    tick_game_ready_sky_cycle, SkyClearColorRuntime, SkyDomeRuntime, SkyVisualKind,
    SkyVisualRuntime,
};
pub(crate) use self::terrain_streaming::{
    tick_game_ready_streaming_terrain, PreparedTerrainPrimitiveMesh, TerrainSurfaceLayers,
};

use self::material_source::*;
use self::materials_terrain::*;
use self::sky::*;
