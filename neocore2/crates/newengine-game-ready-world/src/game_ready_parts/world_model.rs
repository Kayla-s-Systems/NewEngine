#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "world_model/materials.rs"]
mod materials;
#[path = "world_model/spawn.rs"]
mod spawn;
#[path = "world_model/streaming.rs"]
mod streaming;

const STATIC_WORLD_PROXY: &str = "world_static_ydd";
const DYNAMIC_WORLD_PROXY: &str = "world_dynamic_ydd";
const COLLISION_WORLD_PROXY: &str = "world_collision_ydd";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct StaticWorldSpawnSummary {
    pub models: u32,
    pub parts: u32,
    pub triangles: u64,
}

pub(super) use streaming::begin_static_world_prefabs;
pub(crate) use streaming::tick_game_ready_static_world_prefabs;
