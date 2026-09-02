pub(super) fn hair_shadow_receiver_bias(
    shadow_frame: ShadowFrame,
    cascade_index: usize,
    shadow_extent: Extent2D,
) -> f32 {
    let cascade = shadow_frame.cascade(cascade_index);
    let matrix = cascade.light_mvp.to_cols_array();
    // Column-major Mat4 -> geometric rows used to convert normalized depth to world/texel scale.
    let row_x = Vec3::new(matrix[0], matrix[4], matrix[8]);
    let row_y = Vec3::new(matrix[1], matrix[5], matrix[9]);
    let row_z = Vec3::new(matrix[2], matrix[6], matrix[10]);
    let tile_width = if shadow_frame.cascade_count > 1 {
        cascade.viewport.w.max(1.0)
    } else {
        shadow_extent.width.max(1) as f32
    };
    let tile_height = if shadow_frame.cascade_count > 1 {
        cascade.viewport.h.max(1.0)
    } else {
        shadow_extent.height.max(1) as f32
    };
    let world_texel_x = 2.0 / (row_x.length() * tile_width).max(1.0e-6);
    let world_texel_y = 2.0 / (row_y.length() * tile_height).max(1.0e-6);
    let world_texel = world_texel_x.max(world_texel_y);
    let depth_per_texel = (world_texel * row_z.length().max(1.0e-6)).max(1.0e-7);
    let authored_strength = if shadow_frame.params[1] > 0.0 {
        (shadow_frame.params[1] / 0.0025).clamp(0.25, 6.0)
    } else {
        1.0
    };
    (depth_per_texel * 0.65 * authored_strength).clamp(0.000002, 0.002)
}

pub(super) fn encode_shadow_ubo(
    light_view_projection: Mat4,
    directional_dir_intensity: [f32; 4],
    render_segment_count: usize,
    point_base: usize,
    cascade_index: usize,
) -> [u8; HAIR_SHADOW_UBO_BYTES as usize] {
    let mut values = [0.0f32; HAIR_SHADOW_UBO_BYTES as usize / 4];
    values[0..16].copy_from_slice(&light_view_projection.to_cols_array());
    values[16..20].copy_from_slice(&[
        directional_dir_intensity[0],
        directional_dir_intensity[1],
        directional_dir_intensity[2],
        0.0,
    ]);
    values[20..24].copy_from_slice(&[
        render_segment_count as f32,
        point_base as f32,
        SEGMENT_BASE as f32,
        INSTANCE_BASE as f32,
    ]);
    values[24..28].copy_from_slice(&[
        HAIR_INSTANCE_SLOT_COUNT as f32,
        cascade_index as f32,
        0.0,
        0.0,
    ]);
    f32_array_bytes(values)
}
