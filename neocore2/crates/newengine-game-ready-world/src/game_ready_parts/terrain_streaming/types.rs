use super::*;

pub(super) type TerrainChunkCoord = SceneCellCoord;

#[derive(Clone, Debug)]
pub(super) struct TerrainChunkRecord {
    pub(super) terrain: EntityId,
}

#[derive(Clone, Debug)]
pub(super) struct GeneratedTerrainChunk {
    pub(super) terrain: ProceduralTerrain,
    pub(super) mesh: Arc<PrimitiveMesh>,
}

pub(super) struct PendingTerrainChunk {
    pub(super) result: Arc<Mutex<Option<GeneratedTerrainChunk>>>,
    pub(super) ticket: TaskTicket,
}

pub(super) struct GameReadyTerrainStreamingState {
    pub(super) root: EntityId,
    pub(super) anchor: EntityId,
    pub(super) material: MaterialId,
    pub(super) color: [f32; 4],
    pub(super) spec: GameReadyTerrainSpec,
    pub(super) surface: TerrainMaterialLayers,
    pub(super) heightmap: Option<Arc<TerrainHeightmapRuntime>>,
    pub(super) chunk_radius: i32,
    pub(super) unload_radius: i32,
    pub(super) max_chunks_per_frame: usize,
    pub(super) max_pending_jobs: usize,
    pub(super) stream_commit_count: u64,
    pub(super) last_stream_commit_at: Option<Instant>,
    pub(super) loaded: std::collections::BTreeMap<TerrainChunkCoord, TerrainChunkRecord>,
    pub(super) pending: std::collections::BTreeMap<TerrainChunkCoord, PendingTerrainChunk>,
}
