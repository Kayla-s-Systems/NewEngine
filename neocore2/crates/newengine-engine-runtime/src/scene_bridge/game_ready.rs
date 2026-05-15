#![forbid(unsafe_op_in_unsafe_fn)]

mod content;

include!("game_ready_parts/material_source.rs");
include!("game_ready_parts/materials_terrain.rs");
include!("game_ready_parts/foliage/types.rs");
include!("game_ready_parts/foliage/placement.rs");
include!("game_ready_parts/foliage/prefab_loader.rs");
include!("game_ready_parts/foliage/gltf_mesh.rs");
include!("game_ready_parts/foliage/material_binding.rs");
include!("game_ready_parts/foliage/diagnostics.rs");
include!("game_ready_parts/foliage/spawn.rs");
include!("game_ready_parts/assets_bootstrap.rs");
