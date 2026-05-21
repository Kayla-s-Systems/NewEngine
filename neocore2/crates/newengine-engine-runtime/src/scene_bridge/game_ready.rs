#![forbid(unsafe_op_in_unsafe_fn)]

mod content;

use core::f32::consts::TAU;

use newengine_assets::wait_ready;
use newengine_bounds::Bounds;
use newengine_core::{JobLane, JobPriority, JobRequest, JobSystemHandle, JobTicket};
use newengine_ecs::EntityId;
use newengine_lighting::{AmbientLight, DirectionalLight, ShadowSettings};
use newengine_materials::{MaterialFlags, MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Mat4, Quat, Vec3};
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
    spawn_player_controller_with_tuning, FpsDemoRules, FpsDemoState,
    FpsPlayerTuning, GameReadyWorldLaunchGate,
};
use crate::scene_bootstrap::bootstrap_runtime_scene;

use self::content::{
    load_game_ready_map_profile, GameReadyFoliageSpec, GameReadyGameplaySpec,
    GameReadyLightingSpec, GameReadyMaterialSetSpec, GameReadyMaterialSpec,
    GameReadyPaletteSpec, GameReadyPrefabSpec, GameReadySkyAtmosphereSpec,
    GameReadySkySpec, GameReadyTerrainSpec,
};
use super::helpers::{
    apply_exact_material, apply_primitive_instance, ensure_primitive_base, ensure_root, primitive_bounds,
};

include!("game_ready_parts/material_source.rs");
include!("game_ready_parts/sky.rs");
include!("game_ready_parts/materials_terrain.rs");
include!("game_ready_parts/terrain_streaming.rs");
include!("game_ready_parts/foliage/types.rs");
include!("game_ready_parts/foliage/placement.rs");
include!("game_ready_parts/foliage/prefab_loader.rs");
include!("game_ready_parts/foliage/gltf_mesh.rs");
include!("game_ready_parts/foliage/material_binding.rs");
include!("game_ready_parts/foliage/diagnostics.rs");
include!("game_ready_parts/foliage/spawn.rs");
include!("game_ready_parts/player_model.rs");
include!("game_ready_parts/assets_bootstrap.rs");
