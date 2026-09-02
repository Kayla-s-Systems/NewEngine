#[path = "mission/world_items.rs"]
mod world_items;

#[cfg(test)]
use world_items::{
    scaled_world_item_half_extents, world_item_material_asset, world_item_render_options,
};
pub(super) use world_items::{tick_deferred_item_pickups, tick_runtime_world_item_visuals};

include!("mission/assets_materials.rs");
include!("mission/spawn.rs");
include!("mission/character.rs");
include!("mission/instantiate.rs");
include!("mission/tests.rs");
