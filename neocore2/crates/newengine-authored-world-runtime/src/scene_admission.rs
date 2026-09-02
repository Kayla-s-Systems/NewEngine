#![forbid(unsafe_op_in_unsafe_fn)]

mod authored_map_streaming;
mod materials;
mod spawn;
mod streaming;

use newengine_bounds::Bounds;
use newengine_ecs::EntityId;
use newengine_material_domain_api::AuthoredMaterialSpec;
use newengine_material_runtime::authored_registration::{
    is_nemat_entry_ref, register_required_material, register_required_material_ref,
};
use newengine_materials::{MaterialFlags, MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_primitives::{Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_scene::spawn_named;
use newengine_transform::{set_parent, Transform};

use newengine_engine_runtime::world_authoring::{
    spawn_primitive as spawn_game_primitive, PrimitiveSpawnSpec,
};

use crate::{
    AuthoredWorldPlacementSpec, WORLD_COLLISION_BOX_PROXY as BOX_COLLISION_WORLD_PROXY,
    WORLD_COLLISION_PROXY as COLLISION_WORLD_PROXY, WORLD_DYNAMIC_PROXY as DYNAMIC_WORLD_PROXY,
    WORLD_STATIC_PROXY as STATIC_WORLD_PROXY,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthoredStaticWorldSpawnSummary {
    pub models: u32,
    pub parts: u32,
    pub triangles: u64,
}

pub use authored_map_streaming::begin_authored_map_streaming;
pub use authored_map_streaming::tick_authored_map_streaming;
pub use streaming::begin_static_world_prefabs;
pub use streaming::tick_authored_static_world_prefabs;
