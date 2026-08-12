use super::terrain_heightmap::{load_terrain_heightmap, TerrainHeightmapRuntime};
use super::*;
use std::time::{Duration, Instant};

#[path = "terrain_streaming/bootstrap.rs"]
mod bootstrap;
#[path = "terrain_streaming/generation.rs"]
mod generation;
#[path = "terrain_streaming/tick.rs"]
mod tick;
#[path = "terrain_streaming/types.rs"]
mod types;

pub(crate) use newengine_engine_runtime::gameplay::TerrainMaterialLayers;
pub(crate) use tick::tick_game_ready_streaming_terrain;

pub(crate) use bootstrap::spawn_procedural_terrain;

use generation::{
    enqueue_streamed_terrain_chunk, spawn_generated_terrain_chunk, spawn_streamed_terrain_chunk,
    terrain_surface_layers,
};
use types::{
    GameReadyTerrainStreamingState, GeneratedTerrainChunk, PendingTerrainChunk, TerrainChunkCoord,
    TerrainChunkRecord,
};
