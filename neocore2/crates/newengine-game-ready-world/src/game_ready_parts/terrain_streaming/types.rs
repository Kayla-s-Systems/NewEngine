use super::*;

pub(super) type TerrainChunkCoord = SceneCellCoord;

/// Stable sampling snapshot passed directly through the world bootstrap.
///
/// Foliage/player placement must not depend on querying Rust-private ECS component
/// TypeIds after crossing a dynamic-plugin boundary. The generated heightfield is
/// already Arc-backed, so carrying this read-only snapshot is effectively free.
#[derive(Clone, Debug)]
pub(crate) struct TerrainSurfaceSampler {
    pub(crate) origin: Vec3,
    pub(crate) heightfield: Arc<newengine_procedural_noise::HeightField>,
}

impl TerrainSurfaceSampler {
    #[inline]
    pub(crate) fn flat(origin: Vec3, size_x: f32, size_z: f32) -> Self {
        let heightfield = newengine_procedural_noise::HeightField::generate(
            newengine_procedural_noise::TerrainHeightfieldSettings {
                cells_x: 2,
                cells_z: 2,
                size_x,
                size_z,
                base_height: 0.0,
                height_scale: 0.0,
                ..newengine_procedural_noise::TerrainHeightfieldSettings::default()
            },
        );
        Self {
            origin,
            heightfield: Arc::new(heightfield),
        }
    }

    #[inline]
    pub(crate) fn half_extents_xz(&self) -> (f32, f32) {
        let settings = self.heightfield.settings();
        (settings.size_x * 0.5, settings.size_z * 0.5)
    }

    #[inline]
    pub(crate) fn sample_world_height(&self, x: f32, z: f32) -> f32 {
        self.heightfield
            .sample_height_local(x - self.origin.x, z - self.origin.z)
            + self.origin.y
    }
}

#[derive(Clone, Debug)]
pub(super) struct TerrainChunkRecord {
    pub(super) terrain: EntityId,
    pub(super) sampler: TerrainSurfaceSampler,
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
