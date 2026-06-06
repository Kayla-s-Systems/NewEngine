#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "passes_parts/mesh_visibility.rs"]
pub(super) mod mesh_visibility;
#[path = "passes_parts/mesh_passes.rs"]
mod mesh_passes;

pub(super) use self::mesh_passes::{
    draw_primitives, draw_primitives_shadow, draw_procedural_terrain,
    draw_procedural_terrain_shadow, publish_camera_spawn,
};
