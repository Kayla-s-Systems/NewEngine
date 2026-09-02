#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_authored_world_runtime::AuthoredWorldPlacementSpec;
use newengine_bounds::Bounds;
use newengine_ecs::EntityId;
use newengine_engine_runtime::world_authoring::{
    spawn_primitive as spawn_game_primitive, PrimitiveSpawnSpec,
};
use newengine_material_runtime::authored_registration::{
    is_nemat_entry_ref, load_authored_material_descriptor_asset,
};
use newengine_materials::{MaterialId, MaterialRegistry};
use newengine_math::{Quat, Vec3};
use newengine_model_runtime::ydd_runtime::{decode_runtime_ydd_prefab, recompute_ydd_mesh_bounds};
use newengine_primitives::{
    fnv1a_64, PrimitiveId, PrimitiveMesh, PrimitiveRegistry, PrimitiveVertex,
};
use newengine_procedural_noise::ProceduralTerrain;
use newengine_scene::spawn_named;
use newengine_transform::{set_parent, Transform};
use newengine_world_environment_api::authored_profile::{
    AuthoredEnvironmentMaterialSetSpec, AuthoredEnvironmentPaletteSpec, AuthoredFoliageSpec,
};

use crate::authored_materials::AuthoredEnvironmentMaterials;
use crate::terrain_streaming::TerrainSurfaceSampler;

#[path = "authored_foliage/diagnostics.rs"]
mod diagnostics;
#[path = "authored_foliage/material_binding.rs"]
mod material_binding;
#[path = "authored_foliage/placement.rs"]
mod placement;
#[path = "authored_foliage/spawn.rs"]
mod spawn;
#[path = "authored_foliage/types.rs"]
mod types;

use self::diagnostics::*;
use self::material_binding::*;
use self::placement::{choose_foliage_prefab, collect_tree_placements, effective_foliage_spec};
use self::types::*;

#[inline]
fn canonical_ydd_prefab_ref(prefab: &AuthoredWorldPlacementSpec) -> Result<String, String> {
    let source = prefab.source.trim().replace('\\', "/");
    if source.is_empty() {
        return Err(format!(
            "prefab id='{}' has no .ydd@entry source",
            prefab.id
        ));
    }
    if !source.to_ascii_lowercase().contains(".ydd@") {
        return Err(format!(
            "prefab id='{}' source='{}' rejected: runtime requires binary .ydd@entry",
            prefab.id, source
        ));
    }
    Ok(source)
}

pub use self::placement::terrain_height;
pub use self::spawn::{
    defer_foliage_prefabs, spawn_foliage_prefabs, tick_deferred_foliage_prefabs,
};
