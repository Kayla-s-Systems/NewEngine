#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "world_model/authored_map_streaming.rs"]
mod authored_map_streaming;
#[path = "world_model/materials.rs"]
mod materials;
#[path = "world_model/spawn.rs"]
mod spawn;
#[path = "world_model/streaming.rs"]
mod streaming;

use newengine_authored_world_runtime::{
    WORLD_COLLISION_BOX_PROXY as BOX_COLLISION_WORLD_PROXY,
    WORLD_COLLISION_PROXY as COLLISION_WORLD_PROXY, WORLD_DYNAMIC_PROXY as DYNAMIC_WORLD_PROXY,
    WORLD_STATIC_PROXY as STATIC_WORLD_PROXY,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct GroundPlacementSurface;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct StaticWorldSpawnSummary {
    pub models: u32,
    pub parts: u32,
    pub triangles: u64,
}

pub(super) use authored_map_streaming::begin_authored_map_streaming;
pub(crate) use authored_map_streaming::tick_authored_map_streaming;
pub(super) use streaming::begin_static_world_prefabs;
pub(crate) use streaming::tick_game_ready_static_world_prefabs;
