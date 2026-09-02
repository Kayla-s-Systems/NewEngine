use super::terrain_heightmap::{load_terrain_heightmap, TerrainHeightmapRuntime};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use newengine_bounds::Bounds;
use newengine_core::{TaskLane, TaskPriority, TaskRequest, TaskTicket, ThreadPoolHandle};
use newengine_ecs::EntityId;
use newengine_engine_runtime::world_authoring::{apply_exact_material, validate_scene_objects};
use newengine_materials::{MaterialId, MaterialRegistry};
use newengine_math::{Quat, Vec3};
use newengine_primitives::PrimitiveMesh;
use newengine_procedural_noise::{
    DomainWarp2D, NoiseAlgorithm, NoiseCombineMode, NoiseDomain2D, NoiseGraph2D, NoiseLayer2D,
    NoiseRemap, NoiseShape, ProceduralTerrain, TerrainHeightfieldDescriptor,
};
use newengine_scene::{
    spawn_named, SceneBucketedCellPlan, SceneCellCoord, SceneLayeredStreamingPlan,
    SceneResidencySet, SceneStreamingBudget, SceneStreamingObserver, SceneStreamingProfile,
};
use newengine_task_api::{task_domain, task_pass};
use newengine_transform::{set_parent, Transform};
use newengine_world_environment_api::authored_profile::AuthoredTerrainSpec;

#[path = "terrain_streaming/bootstrap.rs"]
mod bootstrap;
#[path = "terrain_streaming/generation.rs"]
mod generation;
#[path = "terrain_streaming/tick.rs"]
mod tick;
#[path = "terrain_streaming/types.rs"]
mod types;

pub(crate) use newengine_engine_runtime::gameplay::TerrainMaterialLayers;
pub use tick::tick_authored_streaming_terrain;
pub use types::TerrainSurfaceSampler;

pub use bootstrap::spawn_procedural_terrain;

use generation::{
    enqueue_streamed_terrain_chunk, spawn_generated_terrain_chunk, spawn_streamed_terrain_chunk,
    terrain_surface_layers,
};
use types::{
    AuthoredTerrainStreamingState, GeneratedTerrainChunk, PendingTerrainChunk, TerrainChunkCoord,
    TerrainChunkRecord,
};
