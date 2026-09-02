use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TreePlacement {
    pub(crate) index: u32,
    pub(crate) position: Vec3,
    pub(crate) yaw: f32,
    pub(crate) scale: f32,
}

pub(crate) struct RuntimePrefabMeshPart {
    pub(crate) primitive_id: PrimitiveId,
    pub(crate) material_slot: String,
    pub(crate) material_id: MaterialId,
    pub(crate) color: [f32; 4],
}

#[derive(Clone)]
pub(crate) struct DeferredFoliageSpawn {
    pub(crate) root: EntityId,
    pub(crate) terrain: EntityId,
    pub(crate) terrain_surface: Option<TerrainSurfaceSampler>,
    pub(crate) materials: AuthoredEnvironmentMaterials,
    pub(crate) material_specs: AuthoredEnvironmentMaterialSetSpec,
    pub(crate) palette: AuthoredEnvironmentPaletteSpec,
    pub(crate) foliage: AuthoredFoliageSpec,
    pub(crate) prefabs: Vec<AuthoredWorldPlacementSpec>,
    pub(crate) player_start: Vec3,
}
