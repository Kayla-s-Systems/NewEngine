use newengine_game_data::default_game_data;

pub(in super::super) fn default_terrain_seed() -> u64 {
    default_game_data().world.terrain.seed
}
pub(in super::super) fn default_terrain_cells() -> u32 {
    default_game_data().world.terrain.cells
}
pub(in super::super) fn default_terrain_size() -> f32 {
    default_game_data().world.terrain.size
}
pub(in super::super) fn default_base_height() -> f32 {
    default_game_data().world.terrain.base_height
}
pub(in super::super) fn default_height_scale() -> f32 {
    default_game_data().world.terrain.height_scale
}
pub(in super::super) fn default_terrain_generator_id() -> String {
    default_game_data().world.terrain.generator.id.clone()
}
pub(in super::super) fn default_ridged_seed_xor() -> u64 {
    default_game_data().world.terrain.generator.ridged_seed_xor
}
pub(in super::super) fn default_ridged_frequency() -> f32 {
    default_game_data().world.terrain.generator.ridged_frequency
}
pub(in super::super) fn default_ridged_amplitude() -> f32 {
    default_game_data().world.terrain.generator.ridged_amplitude
}
pub(in super::super) fn default_ridged_shape_edge0() -> f32 {
    default_game_data()
        .world
        .terrain
        .generator
        .ridged_shape_edge0
}
pub(in super::super) fn default_ridged_shape_edge1() -> f32 {
    default_game_data()
        .world
        .terrain
        .generator
        .ridged_shape_edge1
}
pub(in super::super) fn default_veins_seed_xor() -> u64 {
    default_game_data().world.terrain.generator.veins_seed_xor
}
pub(in super::super) fn default_veins_frequency() -> f32 {
    default_game_data().world.terrain.generator.veins_frequency
}
pub(in super::super) fn default_veins_amplitude() -> f32 {
    default_game_data().world.terrain.generator.veins_amplitude
}
pub(in super::super) fn default_smoothing_passes() -> u32 {
    default_game_data().world.terrain.generator.smoothing_passes
}
pub(in super::super) fn default_smoothing_strength() -> f32 {
    default_game_data()
        .world
        .terrain
        .generator
        .smoothing_strength
}
pub(in super::super) fn default_terrain_surface_forest() -> String {
    default_game_data()
        .world
        .terrain
        .surface
        .forest_texture
        .clone()
}
pub(in super::super) fn default_terrain_surface_sand() -> String {
    default_game_data()
        .world
        .terrain
        .surface
        .sand_texture
        .clone()
}
pub(in super::super) fn default_terrain_surface_rock() -> String {
    default_game_data()
        .world
        .terrain
        .surface
        .rock_texture
        .clone()
}
pub(in super::super) fn default_terrain_patch_scale() -> f32 {
    default_game_data().world.terrain.surface.patch_scale
}
pub(in super::super) fn default_terrain_blend_softness() -> f32 {
    default_game_data().world.terrain.surface.blend_softness
}
pub(in super::super) fn default_terrain_surface_layer_weight() -> f32 {
    default_game_data().world.terrain.surface.layer_weight
}
pub(in super::super) fn default_terrain_surface_layer_uv_scale() -> f32 {
    default_game_data().world.terrain.surface.layer_uv_scale
}
pub(in super::super) fn default_terrain_heightmap_mode() -> String {
    default_game_data().world.terrain.heightmap.mode.clone()
}
pub(in super::super) fn default_terrain_heightmap_strength() -> f32 {
    default_game_data().world.terrain.heightmap.strength
}
pub(in super::super) fn default_terrain_heightmap_min_height() -> f32 {
    default_game_data().world.terrain.heightmap.min_height
}
pub(in super::super) fn default_terrain_heightmap_max_height() -> f32 {
    default_game_data().world.terrain.heightmap.max_height
}
pub(in super::super) fn default_terrain_heightmap_tile_scale() -> [f32; 2] {
    default_game_data().world.terrain.heightmap.tile_scale
}
pub(in super::super) fn default_terrain_heightmap_tile_offset() -> [f32; 2] {
    default_game_data().world.terrain.heightmap.tile_offset
}
pub(in super::super) fn default_terrain_streaming_enabled() -> bool {
    default_game_data().world.terrain.streaming.enabled
}
pub(in super::super) fn default_terrain_chunk_radius() -> i32 {
    default_game_data().world.terrain.streaming.chunk_radius
}
pub(in super::super) fn default_terrain_unload_radius() -> i32 {
    default_game_data().world.terrain.streaming.unload_radius
}
pub(in super::super) fn default_terrain_max_chunks_per_frame() -> usize {
    default_game_data()
        .world
        .terrain
        .streaming
        .max_chunks_per_frame
}
