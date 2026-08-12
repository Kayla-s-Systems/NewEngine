pub(in super::super) fn default_terrain_seed() -> u64 {
    0x2026_0509_4b41_594c
}
pub(in super::super) fn default_terrain_cells() -> u32 {
    80
}
pub(in super::super) fn default_terrain_size() -> f32 {
    52.0
}
pub(in super::super) fn default_base_height() -> f32 {
    -0.04
}
pub(in super::super) fn default_height_scale() -> f32 {
    1.35
}
pub(in super::super) fn default_terrain_generator_id() -> String {
    "newengine.generator.lowland-biomes.v1".to_owned()
}
pub(in super::super) fn default_ridged_seed_xor() -> u64 {
    0x7e22_a11d
}
pub(in super::super) fn default_ridged_frequency() -> f32 {
    1.25
}
pub(in super::super) fn default_ridged_amplitude() -> f32 {
    0.11
}
pub(in super::super) fn default_ridged_shape_edge0() -> f32 {
    0.08
}
pub(in super::super) fn default_ridged_shape_edge1() -> f32 {
    1.0
}
pub(in super::super) fn default_veins_seed_xor() -> u64 {
    0x5317_1001
}
pub(in super::super) fn default_veins_frequency() -> f32 {
    0.52
}
pub(in super::super) fn default_veins_amplitude() -> f32 {
    0.10
}
pub(in super::super) fn default_smoothing_passes() -> u32 {
    2
}
pub(in super::super) fn default_smoothing_strength() -> f32 {
    0.42
}
pub(in super::super) fn default_terrain_surface_forest() -> String {
    String::new()
}
pub(in super::super) fn default_terrain_surface_sand() -> String {
    String::new()
}
pub(in super::super) fn default_terrain_surface_rock() -> String {
    String::new()
}
pub(in super::super) fn default_terrain_patch_scale() -> f32 {
    0.033
}
pub(in super::super) fn default_terrain_blend_softness() -> f32 {
    0.18
}
pub(in super::super) fn default_terrain_surface_layer_weight() -> f32 {
    1.0
}
pub(in super::super) fn default_terrain_surface_layer_uv_scale() -> f32 {
    1.0
}
pub(in super::super) fn default_terrain_heightmap_mode() -> String {
    "blend".to_owned()
}
pub(in super::super) fn default_terrain_heightmap_strength() -> f32 {
    0.0
}
pub(in super::super) fn default_terrain_heightmap_min_height() -> f32 {
    -1.0
}
pub(in super::super) fn default_terrain_heightmap_max_height() -> f32 {
    1.0
}
pub(in super::super) fn default_terrain_heightmap_tile_scale() -> [f32; 2] {
    [1.0, 1.0]
}
pub(in super::super) fn default_terrain_heightmap_tile_offset() -> [f32; 2] {
    [0.0, 0.0]
}
pub(in super::super) fn default_terrain_streaming_enabled() -> bool {
    true
}
pub(in super::super) fn default_terrain_chunk_radius() -> i32 {
    2
}
pub(in super::super) fn default_terrain_unload_radius() -> i32 {
    4
}
pub(in super::super) fn default_terrain_max_chunks_per_frame() -> usize {
    4
}
